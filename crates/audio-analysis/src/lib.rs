pub mod replay;
use anyhow::{Result, ensure};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};
use std::{f64::consts::TAU, sync::Arc};

const FAST: usize = 1024;
const DETAIL: usize = 4096;
const HOP: usize = 256;

/// One-sided, normalized PCM power, retaining independent channels and DC.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PcmSpectrum {
    pub fft_size: u32,
    pub bin_hz: f64,
    pub nyquist_hz: f64,
    pub end_frame: u64,
    pub capture_end_ns: u64,
    pub left_power: Arc<[f64]>,
    pub right_power: Option<Arc<[f64]>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Snapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_spectrum: Option<PcmSpectrum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_spectrum: Option<PcmSpectrum>,
    pub generation: u64,
    pub end_frame: u64,
    pub capture_end_ns: u64,
    pub timestamp_estimated: bool,
    pub sample_rate: u32,
    pub channels: u16,
    pub rms: f64,
    pub peak: f64,
    pub left_rms: f64,
    pub right_rms: Option<f64>,
    pub balance: Option<f64>,
    pub correlation: Option<f64>,
    pub bass_energy: f64,
    pub sub_bass_energy: f64,
    pub bass_flux: f64,
    pub onset_id: u64,
    pub onset_frame: Option<u64>,
    pub bands: Vec<f64>,
    pub band_bin_counts: Vec<u16>,
    pub chroma: [f64; 12],
    pub tonal_valid: bool,
    pub tonal_confidence: f64,
    pub tonal_change: f64,
    pub fast_window_frames: u32,
    pub envelope_window_frames: u32,
    pub detail_window_frames: u32,
    pub detail_end_frame: Option<u64>,
    pub discontinuities: u64,
}

struct Spectrum {
    fft: Arc<dyn Fft<f64>>,
    window: Vec<f64>,
    input: Vec<Complex<f64>>,
    scratch: Vec<Complex<f64>>,
    power: Vec<f64>,
    raw_power: [Vec<f64>; 2],
    normalization: f64,
}

impl Spectrum {
    fn new(n: usize, planner: &mut FftPlanner<f64>) -> Self {
        let fft = planner.plan_fft_forward(n);
        let window: Vec<f64> = (0..n)
            .map(|i| 0.5 - 0.5 * (TAU * i as f64 / n as f64).cos())
            .collect();
        let normalization = 1.0 / (n as f64 * window.iter().map(|v| v * v).sum::<f64>());
        Self {
            scratch: vec![Complex::default(); fft.get_inplace_scratch_len()],
            fft,
            window,
            input: vec![Complex::default(); n],
            power: vec![0.0; n / 2 + 1],
            raw_power: [vec![0.0; n / 2 + 1], vec![0.0; n / 2 + 1]],
            normalization,
        }
    }

    fn analyze(&mut self, history: &[[f64; 2]], end: usize, channels: usize) {
        self.power.fill(0.0);
        let n = self.input.len();
        for channel in [0, 1].into_iter().take(channels) {
            // Remove only the window DC component before spectral analysis.
            // Raw waveform RMS/peak preserve the original PCM gain and offset.
            let mean = (0..n)
                .map(|i| history[(end + DETAIL - n + i) % DETAIL][channel])
                .sum::<f64>()
                / n as f64;
            for i in 0..n {
                self.input[i] = Complex::new(
                    (history[(end + DETAIL - n + i) % DETAIL][channel] - mean) * self.window[i],
                    0.0,
                );
            }
            self.fft
                .process_with_scratch(&mut self.input, &mut self.scratch);
            for (i, p) in self.power.iter_mut().enumerate() {
                let scale = if i == 0 || i == n / 2 { 1.0 } else { 2.0 };
                *p += self.input[i].norm_sqr() * self.normalization * scale / channels as f64;
                // Restore the removed input DC for the public full PCM spectrum.
                // Periodic Hann's DFT is n/2 at0 and -n/4 at ±1; no second FFT
                // or altered comparison/onset calculation is needed.
                let mut original = self.input[i];
                if i == 0 {
                    original.re += mean * n as f64 * 0.5;
                } else if i == 1 {
                    original.re -= mean * n as f64 * 0.25;
                }
                self.raw_power[channel][i] = original.norm_sqr() * self.normalization * scale;
            }
        }
    }

    fn snapshot(
        &self,
        rate: u32,
        channels: usize,
        end_frame: u64,
        capture_end_ns: u64,
    ) -> PcmSpectrum {
        PcmSpectrum {
            fft_size: self.input.len() as u32,
            bin_hz: rate as f64 / self.input.len() as f64,
            nyquist_hz: rate as f64 * 0.5,
            end_frame,
            capture_end_ns,
            left_power: Arc::from(self.raw_power[0].as_slice()),
            right_power: (channels == 2).then(|| Arc::from(self.raw_power[1].as_slice())),
        }
    }
}

/// Continuous signed PCM. No normalization/AGC, source inference or stem claims.
/// Stereo spectral power averages channel powers, preserving antiphase signals.
pub struct Analyzer {
    rate: u32,
    channels: usize,
    generation: u64,
    history: Vec<[f64; 2]>,
    write: usize,
    filled: usize,
    expected_frame: Option<u64>,
    since_hop: usize,
    since_detail: usize,
    fast: Spectrum,
    detail: Spectrum,
    previous_fast: Vec<f64>,
    previous_chroma: [f64; 12],
    snapshot: Snapshot,
    last_onset: Option<u64>,
}

impl Analyzer {
    pub fn new(sample_rate: u32, channels: u16, generation: u64) -> Result<Self> {
        ensure!(
            (8000..=192000).contains(&sample_rate),
            "unsupported sample rate {sample_rate}"
        );
        ensure!((1..=2).contains(&channels), "expected mono or stereo PCM");
        let mut planner = FftPlanner::new();
        Ok(Self {
            rate: sample_rate,
            channels: channels as usize,
            generation,
            history: vec![[0.0; 2]; DETAIL],
            write: 0,
            filled: 0,
            expected_frame: None,
            since_hop: 0,
            since_detail: 0,
            fast: Spectrum::new(FAST, &mut planner),
            detail: Spectrum::new(DETAIL, &mut planner),
            previous_fast: vec![0.0; FAST / 2 + 1],
            previous_chroma: [0.0; 12],
            last_onset: None,
            snapshot: Snapshot {
                generation,
                sample_rate,
                channels,
                fast_window_frames: FAST as u32,
                envelope_window_frames: HOP as u32,
                detail_window_frames: DETAIL as u32,
                bands: vec![0.0; 64],
                band_bin_counts: vec![0; 64],
                ..Snapshot::default()
            },
        })
    }

    pub fn push_i16(
        &mut self,
        pcm: &[i16],
        start_frame: u64,
        end_ns: u64,
        estimated: bool,
    ) -> Result<Option<Snapshot>> {
        ensure!(
            pcm.len().is_multiple_of(self.channels),
            "partial interleaved PCM frame"
        );
        let frames = pcm.len() / self.channels;
        ensure!(
            start_frame.checked_add(frames as u64).is_some(),
            "sample frame overflow"
        );
        if self.expected_frame.is_some_and(|f| f != start_frame) {
            self.reset_history();
        }
        let mut produced = false;
        for (index, f) in pcm.chunks_exact(self.channels).enumerate() {
            self.history[self.write] = [
                f[0] as f64 / 32768.0,
                if self.channels == 2 {
                    f[1] as f64 / 32768.0
                } else {
                    0.0
                },
            ];
            self.write = (self.write + 1) % DETAIL;
            self.filled = (self.filled + 1).min(DETAIL);
            self.since_hop += 1;
            self.since_detail += 1;
            let end = start_frame + index as u64 + 1;
            if self.since_hop == HOP {
                self.since_hop = 0;
                let offset_ns = ((frames - index - 1) as u64 * 1_000_000_000) / self.rate as u64;
                self.update(end, end_ns.saturating_sub(offset_ns), estimated);
                produced = true;
            }
        }
        self.expected_frame = Some(start_frame + frames as u64);
        Ok(produced.then(|| self.snapshot.clone()))
    }

    fn reset_history(&mut self) {
        self.history.fill([0.0; 2]);
        self.filled = 0;
        self.write = 0;
        self.since_hop = 0;
        self.since_detail = 0;
        self.previous_fast.fill(0.0);
        self.previous_chroma.fill(0.0);
        self.last_onset = None;
        let discontinuities = self.snapshot.discontinuities + 1;
        let onset_id = self.snapshot.onset_id;
        self.snapshot = Snapshot {
            generation: self.generation,
            sample_rate: self.rate,
            channels: self.channels as u16,
            bands: vec![0.0; 64],
            band_bin_counts: vec![0; 64],
            fast_window_frames: FAST as u32,
            envelope_window_frames: HOP as u32,
            detail_window_frames: DETAIL as u32,
            discontinuities,
            onset_id,
            ..Snapshot::default()
        };
    }

    fn update(&mut self, end_frame: u64, end_ns: u64, estimated: bool) {
        let mut sums = [0.0; 2];
        let mut product = 0.0;
        let mut peak: f64 = 0.0;
        for i in 0..HOP {
            let v = self.history[(self.write + DETAIL - HOP + i) % DETAIL];
            for c in 0..self.channels {
                sums[c] += v[c] * v[c];
                peak = peak.max(v[c].abs());
            }
            product += v[0] * v[1];
        }
        let s = &mut self.snapshot;
        s.end_frame = end_frame;
        s.capture_end_ns = end_ns;
        s.timestamp_estimated = estimated;
        s.peak = peak;
        s.left_rms = (sums[0] / HOP as f64).sqrt();
        s.rms = (sums.iter().sum::<f64>() / (HOP * self.channels) as f64).sqrt();
        if self.channels == 2 {
            let right = (sums[1] / HOP as f64).sqrt();
            s.right_rms = Some(right);
            let total = s.left_rms + right;
            s.balance = (total > 1e-12).then(|| (right - s.left_rms) / total);
            s.correlation = (sums[0] * sums[1] > 1e-24)
                .then(|| (product / (sums[0] * sums[1]).sqrt()).clamp(-1.0, 1.0));
        }
        s.onset_frame = None;
        if self.filled >= FAST {
            self.fast.analyze(&self.history, self.write, self.channels);
            s.fast_spectrum = Some(
                self.fast
                    .snapshot(self.rate, self.channels, end_frame, end_ns),
            );
            let bin_hz = self.rate as f64 / FAST as f64;
            let mut flux = 0.0;
            let mut bass = 0.0;
            let mut sub = 0.0;
            for (i, p) in self.fast.power.iter().enumerate() {
                let hz = i as f64 * bin_hz;
                if (20.0..250.0).contains(&hz) {
                    flux += (p.sqrt() - self.previous_fast[i].sqrt()).max(0.0);
                    if hz < 60.0 {
                        sub += p;
                    } else {
                        bass += p;
                    }
                }
                self.previous_fast[i] = *p;
            }
            s.bass_flux = flux;
            s.bass_energy = bass;
            s.sub_bass_energy = sub;
            if flux > 0.015
                && self
                    .last_onset
                    .is_none_or(|f| end_frame.saturating_sub(f) > self.rate as u64 * 80 / 1000)
            {
                s.onset_id += 1;
                s.onset_frame = Some(end_frame);
                self.last_onset = Some(end_frame);
            }
        }
        if self.filled >= DETAIL && self.since_detail >= 1024 {
            self.since_detail = 0;
            self.detail
                .analyze(&self.history, self.write, self.channels);
            s.detail_spectrum =
                Some(
                    self.detail
                        .snapshot(self.rate, self.channels, end_frame, end_ns),
                );
            s.bands.fill(0.0);
            s.band_bin_counts.fill(0);
            s.chroma.fill(0.0);
            let max_hz = 20000.0_f64.min(self.rate as f64 / 2.0);
            let ratio = max_hz / 20.0;
            let bin_hz = self.rate as f64 / DETAIL as f64;
            let mut total = 0.0;
            let mut maximum: f64 = 0.0;
            for (i, p) in self.detail.power.iter().enumerate().skip(1) {
                let hz = i as f64 * bin_hz;
                if (20.0..=max_hz).contains(&hz) {
                    let band = ((hz / 20.0).ln() / ratio.ln() * 64.0).floor() as usize;
                    s.bands[band.min(63)] += p;
                    s.band_bin_counts[band.min(63)] += 1;
                }
                if (40.0..5000.0).contains(&hz) {
                    total += p;
                    maximum = maximum.max(*p);
                    // Refine local spectral peaks before pitch-class mapping.
                    // Nearest raw bins misclassify low notes at this resolution.
                    if i + 1 < self.detail.power.len()
                        && *p > self.detail.power[i - 1]
                        && *p >= self.detail.power[i + 1]
                    {
                        let left = self.detail.power[i - 1].max(1e-30).ln();
                        let center = p.max(1e-30).ln();
                        let right = self.detail.power[i + 1].max(1e-30).ln();
                        let denominator = left - 2.0 * center + right;
                        let offset = if denominator.abs() > 1e-15 {
                            (0.5 * (left - right) / denominator).clamp(-0.5, 0.5)
                        } else {
                            0.0
                        };
                        let refined_hz = (i as f64 + offset) * bin_hz;
                        let note = (69.0 + 12.0 * (refined_hz / 440.0).log2()).round() as i32;
                        s.chroma[note.rem_euclid(12) as usize] += p;
                    }
                }
            }
            s.tonal_valid = total > 1e-8 && maximum / total > 0.03;
            s.tonal_confidence = if s.tonal_valid {
                (maximum / total).clamp(0.0, 1.0)
            } else {
                0.0
            };
            if s.tonal_valid {
                let peak_total = s.chroma.iter().sum::<f64>();
                for v in &mut s.chroma {
                    *v /= peak_total.max(1e-30);
                }
                s.tonal_change = s
                    .chroma
                    .iter()
                    .zip(self.previous_chroma)
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f64>()
                    / 2.0;
                self.previous_chroma = s.chroma;
            } else {
                s.chroma.fill(0.0);
                s.tonal_change = 0.0;
                self.previous_chroma.fill(0.0);
            }
            s.detail_end_frame = Some(end_frame);
        }
    }
}
