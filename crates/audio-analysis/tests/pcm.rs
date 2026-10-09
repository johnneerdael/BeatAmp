use audio_analysis::Analyzer;
use std::f64::consts::TAU;

fn stereo(frames: usize, rate: u32, frequency: f64, amplitude: f64, right: f64) -> Vec<i16> {
    (0..frames)
        .flat_map(|i| {
            let s = amplitude * (TAU * frequency * i as f64 / rate as f64).sin();
            [(s * 32767.0) as i16, (s * right * 32767.0) as i16]
        })
        .collect()
}

#[test]
fn continuous_pcm_has_real_stereo_and_sample_identity() {
    let mut a = Analyzer::new(48000, 2, 7).unwrap();
    let pcm = stereo(8192, 48000, 110.0, 0.5, 0.25);
    let expected_rms = (pcm[pcm.len() - 512..]
        .chunks_exact(2)
        .map(|f| (f[0] as f64 / 32768.0).powi(2))
        .sum::<f64>()
        / 256.0)
        .sqrt();
    let s = a.push_i16(&pcm, 0, 2_000_000_000, false).unwrap().unwrap();
    assert_eq!(
        (s.generation, s.sample_rate, s.channels, s.end_frame),
        (7, 48000, 2, 8192)
    );
    assert!((s.left_rms - expected_rms).abs() < 1e-12);
    assert!((s.right_rms.unwrap() / s.left_rms - 0.25).abs() < 0.005);
    assert!(s.balance.unwrap() < -0.59);
    assert!(s.correlation.unwrap() > 0.999);
    assert!(s.bass_energy > s.sub_bass_energy * 10.0);
    assert!(!s.timestamp_estimated);
}

#[test]
fn mono_never_invents_stereo() {
    let mut a = Analyzer::new(44100, 1, 1).unwrap();
    let s = a
        .push_i16(&vec![0; 8192], 0, 1_000_000_000, true)
        .unwrap()
        .unwrap();
    assert_eq!(s.channels, 1);
    assert!(s.right_rms.is_none() && s.balance.is_none() && s.correlation.is_none());
    assert!(!s.tonal_valid && s.onset_frame.is_none());
    assert_eq!(s.rms, 0.0);
}

#[test]
fn chunk_boundaries_preserve_features_and_event_identity() {
    let pcm = stereo(8192, 48000, 440.0, 0.4, 1.0);
    let mut full = Analyzer::new(48000, 2, 2).unwrap();
    let expected = full
        .push_i16(&pcm, 0, 3_000_000_000, false)
        .unwrap()
        .unwrap();
    let mut chunked = Analyzer::new(48000, 2, 2).unwrap();
    let mut actual = None;
    for (i, c) in pcm.chunks(274).enumerate() {
        let start = i as u64 * 137;
        let end = start + (c.len() / 2) as u64;
        let ns = 3_000_000_000 - (8192 - end) * 1_000_000_000 / 48000;
        if let Some(s) = chunked.push_i16(c, start, ns, false).unwrap() {
            actual = Some(s);
        }
    }
    let actual = actual.unwrap();
    assert_eq!(expected.end_frame, actual.end_frame);
    assert_eq!(expected.onset_id, actual.onset_id);
    assert_eq!(expected.chroma, actual.chroma);
    assert_eq!(expected.bands, actual.bands);
    assert!((expected.capture_end_ns as i64 - actual.capture_end_ns as i64).abs() <= 1);
    assert!(actual.tonal_valid);
    assert_eq!(
        actual
            .chroma
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0,
        9
    );
}

#[test]
fn left_and_right_phase_cancellation_does_not_hide_energy() {
    let mut a = Analyzer::new(48000, 2, 1).unwrap();
    let s = a
        .push_i16(
            &stereo(8192, 48000, 110.0, 0.6, -1.0),
            0,
            1_000_000_000,
            false,
        )
        .unwrap()
        .unwrap();
    assert!(s.rms > 0.4 && s.bass_energy > 0.01);
    assert!(s.correlation.unwrap() < -0.999);
}

#[test]
fn bass_amplitude_response_is_monotonic() {
    let mut energies = vec![];
    for amplitude in [0.1, 0.3, 0.8] {
        let mut a = Analyzer::new(48000, 2, 1).unwrap();
        let s = a
            .push_i16(
                &stereo(8192, 48000, 90.0, amplitude, 1.0),
                0,
                1_000_000_000,
                false,
            )
            .unwrap()
            .unwrap();
        energies.push(s.bass_energy);
    }
    assert!(
        energies[0] > 0.0 && energies[1] > energies[0] * 7.0 && energies[2] > energies[1] * 6.0
    );
}

#[test]
fn discontinuity_resets_history_and_rejects_bad_formats() {
    assert!(Analyzer::new(0, 2, 0).is_err());
    assert!(Analyzer::new(48000, 3, 0).is_err());
    let mut a = Analyzer::new(48000, 2, 3).unwrap();
    assert!(a.push_i16(&[1, 2, 3], 0, 1, false).is_err());
    a.push_i16(
        &stereo(8192, 48000, 440.0, 0.8, 1.0),
        0,
        1_000_000_000,
        false,
    )
    .unwrap();
    let s = a
        .push_i16(&vec![0; 8192], 10000, 2_000_000_000, true)
        .unwrap()
        .unwrap();
    assert_eq!(s.discontinuities, 1);
    assert_eq!(s.rms, 0.0);
    assert!(!s.tonal_valid && s.onset_frame.is_none());
}

#[test]
fn detail_analysis_recovers_after_non_aligned_sample_discontinuity() {
    let mut a = Analyzer::new(48000, 2, 1).unwrap();
    a.push_i16(&vec![0; 8192], 0, 1_000_000_000, false).unwrap();
    let s = a
        .push_i16(
            &stereo(8192, 48000, 440.0, 0.5, 1.0),
            10001,
            2_000_000_000,
            false,
        )
        .unwrap()
        .unwrap();
    assert!(s.tonal_valid && s.detail_end_frame.is_some());
}

#[test]
fn chroma_recognizes_low_bass_and_higher_melodic_notes() {
    for (frequency, pitch_class) in [(110.0, 9), (880.0, 9), (1046.502, 0)] {
        let mut a = Analyzer::new(48000, 2, 1).unwrap();
        let s = a
            .push_i16(
                &stereo(8192, 48000, frequency, 0.4, 1.0),
                0,
                1_000_000_000,
                false,
            )
            .unwrap()
            .unwrap();
        assert!(s.tonal_valid);
        assert_eq!(
            s.chroma
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0,
            pitch_class,
            "{frequency} Hz"
        );
    }
}

#[test]
fn dc_offset_is_measured_but_does_not_create_a_bass_onset_or_tone() {
    let mut a = Analyzer::new(48000, 2, 1).unwrap();
    let s = a
        .push_i16(&vec![9000; 16384], 0, 1_000_000_000, false)
        .unwrap()
        .unwrap();
    assert!(s.rms > 0.2);
    assert_eq!(s.bass_energy, 0.0);
    assert_eq!(s.sub_bass_energy, 0.0);
    assert_eq!(s.onset_id, 0);
    assert!(!s.tonal_valid);
}

#[test]
fn full_spectra_preserve_independent_channels_above_the_band_summary_cutoff() {
    // Publishing only20kHz summary bands or downmixing stereo loses this right-channel tone.
    let rate = 48000;
    let pcm: Vec<i16> = (0..8192)
        .flat_map(|i| {
            [
                (0.4 * (TAU * 110.0 * i as f64 / rate as f64).sin() * 32767.0) as i16,
                (0.4 * (TAU * 22000.0 * i as f64 / rate as f64).sin() * 32767.0) as i16,
            ]
        })
        .collect();
    let mut analyzer = Analyzer::new(rate, 2, 9).unwrap();
    let s = analyzer
        .push_i16(&pcm, 0, 2_000_000_000, false)
        .unwrap()
        .unwrap();
    for (spectrum, bins, resolution) in [
        (s.fast_spectrum.as_ref().unwrap(), 513, 46.875),
        (s.detail_spectrum.as_ref().unwrap(), 2049, 11.71875),
    ] {
        assert_eq!(spectrum.left_power.len(), bins);
        assert_eq!(spectrum.right_power.as_ref().unwrap().len(), bins);
        assert_eq!(spectrum.bin_hz, resolution);
        assert_eq!(spectrum.nyquist_hz, 24000.0);
        assert_eq!(spectrum.end_frame, 8192);
        let peak = |values: &[f64]| {
            values
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0 as f64
                * resolution
        };
        assert!((peak(&spectrum.left_power) - 110.0).abs() <= resolution);
        assert!((peak(spectrum.right_power.as_ref().unwrap()) - 22000.0).abs() <= resolution);
        assert!(spectrum.right_power.as_ref().unwrap().iter().sum::<f64>() > 0.07);
    }
}

#[test]
fn full_pcm_spectrum_includes_dc_without_turning_it_into_a_false_bass_onset() {
    // A spectrum reconstructed from DC-removed features alone loses real input offset.
    let mut analyzer = Analyzer::new(48000, 1, 1).unwrap();
    let s = analyzer
        .push_i16(&vec![9000; 8192], 0, 1_000_000_000, false)
        .unwrap()
        .unwrap();
    let spectrum = s.detail_spectrum.unwrap();
    assert!(spectrum.right_power.is_none());
    assert!(spectrum.left_power[0] > 0.04);
    let expected_power = (9000.0_f64 / 32768.0).powi(2);
    assert!((spectrum.left_power.iter().sum::<f64>() - expected_power).abs() < 1e-12);
    assert_eq!(s.bass_flux, 0.0);
    assert_eq!(s.onset_id, 0);
}

#[test]
fn nyquist_bin_is_retained_without_doubling_its_energy() {
    // Dropping the final FFT bin or using the interior-bin factor2 at Nyquist breaks energy.
    let mut analyzer = Analyzer::new(44100, 1, 1).unwrap();
    let pcm: Vec<i16> = (0..8192)
        .map(|i| if i % 2 == 0 { 12000 } else { -12000 })
        .collect();
    let s = analyzer
        .push_i16(&pcm, 0, 1_000_000_000, false)
        .unwrap()
        .unwrap();
    let spectrum = s.detail_spectrum.unwrap();
    assert_eq!(spectrum.nyquist_hz, 22050.0);
    let peak = spectrum
        .left_power
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0;
    assert_eq!(peak, 2048);
    let expected_power = (12000.0_f64 / 32768.0).powi(2);
    assert!((spectrum.left_power.iter().sum::<f64>() - expected_power).abs() < 1e-12);
}
