use anyhow::{Result, ensure};
use audio_analysis::{PcmSpectrum, Snapshot};
use std::collections::BTreeMap;

/// Renderer-independent updates. Buffers contain LE vec4<f32> rows:
/// raw left power, raw right power, left envelope, right envelope.
/// A missing right channel remains zero; availability is separately declared.
#[derive(Clone, Debug, Default)]
pub struct ControlFrame {
    pub parameters: BTreeMap<String, Vec<f32>>,
    pub buffers: BTreeMap<String, Vec<u8>>,
    pub generation: u64,
    pub source_end_frame: u64,
    pub capture_end_ns: u64,
    pub timestamp_estimated: bool,
}
pub struct Controller {
    extent: [u32; 2],
    generation: Option<u64>,
    previous_ns: Option<u64>,
    previous_source: Option<(u64, u64)>,
    start_ns: u64,
    energy: [f32; 4],
    regions: [f32; 8],
    chroma: [f32; 12],
    fast: Vec<[f32; 4]>,
    detail: Vec<[f32; 4]>,
}
fn response(power: f64, gain: f64) -> f32 {
    let amplitude = power.max(0.0).sqrt() * gain;
    (amplitude / (1.0 + amplitude)) as f32
}
fn envelope(old: f32, target: f32, dt: f32) -> f32 {
    let seconds = if target > old { 0.010 } else { 0.180 };
    old + (target - old) * (1.0 - (-dt / seconds).exp())
}
fn encode(rows: &[[f32; 4]]) -> Vec<u8> {
    rows.iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect()
}
fn spectrum_valid(spectrum: &PcmSpectrum, bins: usize, fft: u32, rate: u32) -> Result<()> {
    ensure!(
        spectrum.fft_size == fft
            && spectrum.left_power.len() == bins
            && spectrum
                .right_power
                .as_ref()
                .is_none_or(|v| v.len() == bins),
        "unsupported PCM spectrum dimensions"
    );
    ensure!(
        rate > 0
            && spectrum.bin_hz.is_finite()
            && spectrum.nyquist_hz.is_finite()
            && (spectrum.bin_hz - rate as f64 / fft as f64).abs() < 1e-8
            && (spectrum.nyquist_hz - rate as f64 / 2.0).abs() < 1e-8,
        "invalid spectrum rate/frequency identity"
    );
    ensure!(
        spectrum
            .left_power
            .iter()
            .chain(spectrum.right_power.iter().flat_map(|v| v.iter()))
            .all(|v| v.is_finite() && *v >= 0.0 && *v <= f32::MAX as f64),
        "invalid PCM spectral power"
    );
    Ok(())
}
fn update_spectrum(rows: &mut [[f32; 4]], spectrum: Option<&PcmSpectrum>, dt: f32) {
    for (index, row) in rows.iter_mut().enumerate() {
        let left = spectrum.map_or(0.0, |s| s.left_power[index]);
        let right = spectrum
            .and_then(|s| s.right_power.as_ref())
            .map_or(0.0, |v| v[index]);
        row[0] = left as f32;
        row[1] = right as f32;
        row[2] = envelope(row[2], response(left, 10.0), dt);
        row[3] = envelope(row[3], response(right, 10.0), dt);
    }
}
impl Controller {
    pub fn new(extent: [u32; 2]) -> Result<Self> {
        ensure!(
            extent.iter().all(|v| *v > 0),
            "invalid control native extent"
        );
        Ok(Self {
            extent,
            generation: None,
            previous_ns: None,
            previous_source: None,
            start_ns: 0,
            energy: [0.0; 4],
            regions: [0.0; 8],
            chroma: [0.0; 12],
            fast: vec![[0.0; 4]; 513],
            detail: vec![[0.0; 4]; 2049],
        })
    }
    /// Causal attack10ms/release180ms envelopes; no prediction timeline replay.
    /// Fast1024 analysis does not wait for detail4096 analysis. Availability
    /// expires after250ms fast /500ms detail; stale controls damp toward zero.
    pub fn update(&mut self, snapshot: &Snapshot, now_ns: u64) -> Result<ControlFrame> {
        ensure!(
            snapshot.capture_end_ns <= now_ns,
            "audio capture is newer than the control clock"
        );
        ensure!(
            [
                snapshot.rms,
                snapshot.bass_flux,
                snapshot.bass_energy,
                snapshot.tonal_change,
                snapshot.tonal_confidence
            ]
            .iter()
            .chain(snapshot.chroma.iter())
            .all(|v| v.is_finite() && *v >= 0.0),
            "invalid live audio features"
        );
        ensure!(
            snapshot.balance.is_none_or(|v| v.is_finite())
                && snapshot.correlation.is_none_or(|v| v.is_finite()),
            "invalid stereo features"
        );
        if let Some(s) = &snapshot.fast_spectrum {
            spectrum_valid(s, 513, 1024, snapshot.sample_rate)?;
        }
        if let Some(s) = &snapshot.detail_spectrum {
            spectrum_valid(s, 2049, 4096, snapshot.sample_rate)?;
        }
        for spectrum in [&snapshot.fast_spectrum, &snapshot.detail_spectrum]
            .into_iter()
            .flatten()
        {
            ensure!(
                snapshot.channels == 1 || snapshot.channels == 2,
                "unsupported PCM channel descriptor"
            );
            ensure!(
                snapshot.channels != 1 || spectrum.right_power.is_none(),
                "mono descriptor has right spectral channel"
            );
            ensure!(
                spectrum.end_frame <= snapshot.end_frame
                    && spectrum.capture_end_ns <= snapshot.capture_end_ns,
                "analysis is newer than its source descriptor"
            );
        }
        if self.generation != Some(snapshot.generation) {
            self.generation = Some(snapshot.generation);
            self.previous_ns = None;
            self.previous_source = None;
            self.start_ns = now_ns;
            self.energy.fill(0.0);
            self.regions.fill(0.0);
            self.chroma.fill(0.0);
            self.fast.fill([0.0; 4]);
            self.detail.fill([0.0; 4]);
        }
        ensure!(
            self.previous_ns.is_none_or(|v| now_ns >= v),
            "control clock moved backward within generation"
        );
        ensure!(
            self.previous_source.is_none_or(
                |(frame, time)| snapshot.end_frame >= frame && snapshot.capture_end_ns >= time
            ),
            "audio source descriptor regressed within generation"
        );
        self.previous_source = Some((snapshot.end_frame, snapshot.capture_end_ns));
        let dt = self
            .previous_ns
            .map_or(1.0 / 30.0, |v| ((now_ns - v) as f64 / 1e9).min(2.0) as f32);
        self.previous_ns = Some(now_ns);
        let snapshot_age = now_ns.saturating_sub(snapshot.capture_end_ns) as f64 / 1e9;
        let fast = snapshot
            .fast_spectrum
            .as_ref()
            .filter(|s| now_ns.saturating_sub(s.capture_end_ns) <= 250_000_000);
        let detail = snapshot
            .detail_spectrum
            .as_ref()
            .filter(|s| now_ns.saturating_sub(s.capture_end_ns) <= 500_000_000);
        update_spectrum(&mut self.fast, fast, dt);
        update_spectrum(&mut self.detail, detail, dt);
        let live = snapshot_age <= 0.25;
        let tonal = live && snapshot.tonal_valid && detail.is_some();
        let energy = if live {
            [
                response(snapshot.rms * snapshot.rms, 8.0),
                response(snapshot.bass_flux, 4.0),
                response(snapshot.bass_energy, 5.0),
                if tonal {
                    snapshot.tonal_change.min(1.0) as f32
                } else {
                    0.0
                },
            ]
        } else {
            [0.0; 4]
        };
        for (old, target) in self.energy.iter_mut().zip(energy) {
            *old = envelope(*old, target, dt);
        }
        let mut regions = [0.0f64; 8];
        if let Some(s) = fast {
            for (index, left) in s.left_power.iter().enumerate() {
                let frequency = index as f64 * s.bin_hz;
                let region = if index == 0 {
                    0
                } else if frequency < 90.0 {
                    1
                } else if frequency < 350.0 {
                    2
                } else if frequency < 2500.0 {
                    3
                } else if frequency < 10000.0 {
                    4
                } else {
                    5
                };
                let power = if let Some(right) = &s.right_power {
                    (*left + right[index]) * 0.5
                } else {
                    *left
                };
                regions[region] += power;
            }
        }
        for (old, target) in self.regions.iter_mut().zip(regions) {
            *old = envelope(*old, response(target, 7.0), dt);
        }
        for (old, target) in self.chroma.iter_mut().zip(snapshot.chroma) {
            *old = envelope(*old, if tonal { target.min(1.0) as f32 } else { 0.0 }, dt);
        }
        let mut frame = ControlFrame {
            generation: snapshot.generation,
            source_end_frame: snapshot.end_frame,
            capture_end_ns: snapshot.capture_end_ns,
            timestamp_estimated: snapshot.timestamp_estimated,
            ..Default::default()
        };
        frame.parameters.insert(
            "scene_time".into(),
            vec![
                ((now_ns - self.start_ns) as f64 / 1e9) as f32,
                dt,
                self.extent[0] as f32,
                self.extent[1] as f32,
            ],
        );
        frame
            .parameters
            .insert("audio_energy".into(), self.energy.to_vec());
        frame
            .parameters
            .insert("audio_regions".into(), self.regions.to_vec());
        frame
            .parameters
            .insert("audio_chroma".into(), self.chroma.to_vec());
        frame.parameters.insert(
            "audio_space".into(),
            vec![
                if live {
                    snapshot.balance.unwrap_or(0.0) as f32
                } else {
                    0.0
                },
                if live {
                    snapshot.correlation.unwrap_or(0.0) as f32
                } else {
                    0.0
                },
                if tonal {
                    snapshot.tonal_confidence as f32
                } else {
                    0.0
                },
                if fast.is_some() {
                    snapshot.channels as f32
                } else {
                    0.0
                },
            ],
        );
        frame.parameters.insert(
            "audio_windows".into(),
            vec![
                if fast.is_some() { 513.0 } else { 0.0 },
                if detail.is_some() { 2049.0 } else { 0.0 },
                snapshot.fast_spectrum.as_ref().map_or(-1.0, |s| {
                    now_ns.saturating_sub(s.capture_end_ns) as f32 / 1e9
                }),
                snapshot.detail_spectrum.as_ref().map_or(-1.0, |s| {
                    now_ns.saturating_sub(s.capture_end_ns) as f32 / 1e9
                }),
            ],
        );
        frame.parameters.insert(
            "audio_frequency".into(),
            vec![
                fast.map_or(0.0, |s| s.bin_hz as f32),
                detail.map_or(0.0, |s| s.bin_hz as f32),
                snapshot.sample_rate as f32 * 0.5,
                if fast.is_some_and(|s| s.right_power.is_some()) {
                    1.0
                } else {
                    0.0
                },
            ],
        );
        frame
            .buffers
            .insert("audio_fast".into(), encode(&self.fast));
        frame
            .buffers
            .insert("audio_detail".into(), encode(&self.detail));
        Ok(frame)
    }
}
