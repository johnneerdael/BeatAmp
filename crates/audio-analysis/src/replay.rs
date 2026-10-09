//! Fixed-record comparison replay. This is a file input, never live capture.
use crate::Snapshot;
use anyhow::{Result, ensure};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
pub const HEADER_BYTES: u64 = 32;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ReplayFrame {
    pub source_end: u64,
    pub analysis_end: u64,
    pub bass: f64,
    pub sub: f64,
    pub rms: f64,
    pub peak: f64,
    pub bands: [f64; 64],
    pub wave: [i16; 1024],
}
impl ReplayFrame {
    pub fn from_snapshot(s: &Snapshot, source_end: u64, pcm: &[i16]) -> Result<Self> {
        ensure!(
            s.bands.len() == 64 && pcm.len() >= 1024,
            "replay frame shape mismatch"
        );
        let mut f = Self {
            source_end,
            analysis_end: s.end_frame,
            bass: s.bass_energy,
            sub: s.sub_bass_energy,
            rms: s.rms,
            peak: s.peak,
            bands: [0.0; 64],
            wave: [0; 1024],
        };
        f.bands.copy_from_slice(&s.bands);
        f.wave.copy_from_slice(&pcm[pcm.len() - 1024..]);
        Ok(f)
    }
    pub fn snapshot(&self, rate: u32) -> Snapshot {
        Snapshot {
            generation: 1,
            sample_rate: rate,
            channels: 2,
            end_frame: self.analysis_end,
            capture_end_ns: 2_000_000_000 + self.analysis_end * 1_000_000_000 / rate as u64,
            bass_energy: self.bass,
            sub_bass_energy: self.sub,
            rms: self.rms,
            peak: self.peak,
            bands: self.bands.to_vec(),
            fast_window_frames: 1024,
            detail_window_frames: 4096,
            envelope_window_frames: 256,
            ..Default::default()
        }
    }
    pub fn waveform(&self) -> Vec<[f32; 2]> {
        self.wave
            .chunks_exact(2)
            .map(|f| [f[0] as f32 / 32768.0, f[1] as f32 / 32768.0])
            .collect()
    }
}
pub fn write_header(file: &mut File, count: u64, rate: u32) -> Result<()> {
    ensure!(
        cfg!(target_endian = "little"),
        "replay writer needs little-endian target"
    );
    file.write_all(b"PCMRPL1\0")?;
    file.write_all(&count.to_le_bytes())?;
    for value in [std::mem::size_of::<ReplayFrame>() as u32, rate, 30, 2] {
        file.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}
pub struct ReplayReader {
    file: File,
    pub count: u64,
    pub rate: u32,
    buffer: Vec<u8>,
}
impl ReplayReader {
    pub fn open(path: &Path) -> Result<Self> {
        ensure!(
            cfg!(target_endian = "little"),
            "replay reader needs little-endian target"
        );
        let mut file = File::open(path)?;
        let mut header = [0u8; 32];
        file.read_exact(&mut header)?;
        ensure!(&header[..8] == b"PCMRPL1\0", "unsupported replay magic");
        let count = u64::from_le_bytes(header[8..16].try_into()?);
        let width = u32::from_le_bytes(header[16..20].try_into()?) as usize;
        let rate = u32::from_le_bytes(header[20..24].try_into()?);
        ensure!(
            count > 0
                && count <= 1_000_000
                && width == std::mem::size_of::<ReplayFrame>()
                && [44100, 48000].contains(&rate)
                && u32::from_le_bytes(header[24..28].try_into()?) == 30
                && u32::from_le_bytes(header[28..32].try_into()?) == 2,
            "replay header bounds mismatch"
        );
        ensure!(
            file.metadata()?.len() == HEADER_BYTES + count * width as u64,
            "truncated or oversized replay file"
        );
        Ok(Self {
            file,
            count,
            rate,
            buffer: vec![0; width],
        })
    }
    pub fn read(&mut self, index: u64) -> Result<ReplayFrame> {
        ensure!(index < self.count, "replay index past source timeline");
        self.file.seek(SeekFrom::Start(
            HEADER_BYTES + index * self.buffer.len() as u64,
        ))?;
        self.file.read_exact(&mut self.buffer)?;
        let f: ReplayFrame = bytemuck::pod_read_unaligned(&self.buffer);
        ensure!(
            f.analysis_end <= f.source_end
                && [f.bass, f.sub, f.rms, f.peak]
                    .iter()
                    .chain(f.bands.iter())
                    .all(|v| v.is_finite() && *v >= 0.0),
            "invalid replay features"
        );
        Ok(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexed_replay_preserves_stereo_and_rejects_truncation_and_past_end() {
        let path = std::env::temp_dir().join(format!(
            "preset-replay-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let s = Snapshot {
            end_frame: 1024,
            bands: vec![0.0; 64],
            ..Default::default()
        };
        let mut pcm = vec![0i16; 1024];
        pcm[0] = i16::MIN;
        pcm[1] = i16::MAX;
        let f = ReplayFrame::from_snapshot(&s, 1470, &pcm).unwrap();
        let mut output = File::create(&path).unwrap();
        write_header(&mut output, 2, 44100).unwrap();
        output.write_all(bytemuck::bytes_of(&f)).unwrap();
        output.write_all(bytemuck::bytes_of(&f)).unwrap();
        drop(output);
        let mut reader = ReplayReader::open(&path).unwrap();
        let second = reader.read(1).unwrap();
        assert_eq!(second.waveform()[0], [-1.0, 32767.0 / 32768.0]);
        assert_eq!(second.source_end, 1470);
        assert!(reader.read(2).is_err());
        drop(reader);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(HEADER_BYTES + 1)
            .unwrap();
        assert!(ReplayReader::open(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
