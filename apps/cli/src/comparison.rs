use anyhow::{Result, ensure};
use std::path::Path;

/// Frozen controlled-fixture profile, deliberately not a general WAV decoder.
pub fn prepare(root: &Path, input: &Path) -> Result<()> {
    let raw = std::fs::read(input)?;
    ensure!(
        raw.len() >= 44
            && &raw[0..4] == b"RIFF"
            && &raw[8..16] == b"WAVEfmt "
            && &raw[36..40] == b"data",
        "expected controlled PCM RIFF profile"
    );
    ensure!(
        u16::from_le_bytes(raw[20..22].try_into()?) == 1
            && u16::from_le_bytes(raw[22..24].try_into()?) == 2
            && u32::from_le_bytes(raw[24..28].try_into()?) == 48000
            && u16::from_le_bytes(raw[34..36].try_into()?) == 16,
        "expected stereo48k i16 fixture"
    );
    ensure!(
        raw.len() >= 44 + 480 * 1600 * 4,
        "fixture must contain16 seconds"
    );
    let pcm: Vec<i16> = raw[44..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    let mut analyzer = audio_analysis::Analyzer::new(48000, 2, 1)?;
    let mut frames = vec![];
    let mut reference = vec![];
    let stereo = |position: usize| {
        [
            pcm[position * 2] as f32 / 32768.0,
            pcm[position * 2 + 1] as f32 / 32768.0,
        ]
    };
    for frame in 0..480 {
        let start = frame * 1600;
        let end = start + 1600;
        let snapshot = analyzer
            .push_i16(
                &pcm[start * 2..end * 2],
                start as u64,
                2_000_000_000 + end as u64 * 1_000_000_000 / 48000,
                false,
            )?
            .expect("1024-window snapshot");
        let waveform: Vec<_> = (end - 512..end).map(stereo).collect();
        frames.push(serde_json::json!({"frame":frame,"source_sample_end":end,"snapshot":snapshot,"waveform":waveform}));
        for i in 0..576 {
            let position = end as f64 - (576 - i) as f64 * 48000.0 / 44100.0;
            let index = position.floor() as usize;
            let mix = position.fract() as f32;
            let a = stereo(index);
            let b = stereo(index + 1);
            let mono = ((a[0] + a[1]) * (1.0 - mix) + (b[0] + b[1]) * mix) * 0.5;
            reference.push((128.0 + mono * 127.0).round().clamp(0.0, 255.0) as u8);
        }
    }
    let source_sha = milk_import::corpus::sha256(&raw);
    let identity = serde_json::json!({"source":input.display().to_string(),"source_sha256":source_sha,"source_encoding":"stereo48000Hz signed i16 PCM WAV from actual Milkbeat controlled fixture","fps":30,"frames":480,"candidate_input":"512 stereo samples and causally computed1024/4096 FFT features; end-aligned", "reference_input":"same source sample end;576 samples linear48k-to44.1k resampling, arithmetic stereo downmix, u8 round(128+127*v)","reference_engine_clock_rng":"uncontrolled shipping API; no deterministic equality or fidelity percentage"});
    let json = serde_json::to_vec(&serde_json::json!({"identity":identity,"frames":frames}))?;
    super::write_bytes(root, &root.join("reports/common-audio.json"), &json)?;
    super::write_bytes(
        root,
        &root.join("platform/android/reference/src/main/assets/audio/reference-input.bin"),
        &reference,
    )?;
    super::write_report(
        root,
        &root.join("benchmarks/comparison/common-pcm-v1/identity.json"),
        &serde_json::to_string_pretty(&identity)?,
    )?;
    println!("Prepared480 end-aligned common PCM comparison frames");
    Ok(())
}

pub fn playlist(root: &Path, input: &Path, rate: u32) -> Result<()> {
    use audio_analysis::replay::{ReplayFrame, write_header};
    use std::io::{BufReader, BufWriter, Read, Write};
    ensure!(
        [44100, 48000].contains(&rate),
        "playlist rate must be44100 or48000"
    );
    let samples_per_frame = (rate / 30) as usize;
    let stride = samples_per_frame * 4;
    let length = std::fs::metadata(input)?.len();
    ensure!(
        length > 0 && length.is_multiple_of(stride as u64),
        "playlist must end on a full30fps PCM frame"
    );
    let count = length / stride as u64;
    ensure!(count <= 1_000_000, "playlist replay frame bound exceeded");
    let folder = root.join("benchmarks/music-review");
    std::fs::create_dir_all(&folder)?;
    let mut file = std::fs::File::create(folder.join("playlist.frames"))?;
    write_header(&mut file, count, rate)?;
    let mut output = BufWriter::new(file);
    let mut reference = BufWriter::new(std::fs::File::create(folder.join("reference.bin"))?);
    let mut pcm = BufReader::new(std::fs::File::open(input)?);
    let mut block = vec![0u8; stride];
    let mut analyzer = audio_analysis::Analyzer::new(rate, 2, 1)?;
    for frame in 0..count {
        pcm.read_exact(&mut block)?;
        let samples: Vec<i16> = block
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        let start = frame * samples_per_frame as u64;
        let end = start + samples_per_frame as u64;
        let s = analyzer
            .push_i16(
                &samples,
                start,
                2_000_000_000 + end * 1_000_000_000 / rate as u64,
                false,
            )?
            .expect("snapshot");
        output.write_all(bytemuck::bytes_of(&ReplayFrame::from_snapshot(
            &s, end, &samples,
        )?))?;
        // Current Beatport corpus is stereo44100 i16. Avoid resampling it.
        ensure!(
            rate == 44100,
            "reference playlist derivation currently requires44100 source"
        );
        for pair in samples[samples.len() - 576 * 2..].chunks_exact(2) {
            let mono = (pair[0] as f64 + pair[1] as f64) / 65536.0;
            reference.write_all(&[(128.0 + mono * 127.0).round().clamp(0.0, 255.0) as u8])?;
        }
        if frame % 9000 == 0 {
            println!("Prepared{} /{} replay frames", frame, count);
        }
    }
    output.flush()?;
    reference.flush()?;
    println!("Prepared full{}-second playlist replay", count / 30);
    Ok(())
}
