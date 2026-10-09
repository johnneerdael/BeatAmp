use audio_analysis::{Analyzer, Snapshot};
use beat_authoring::Controller;
fn tone(hz: f64, right: f64) -> Snapshot {
    let mut analyzer = Analyzer::new(48_000, 2, 1).unwrap();
    let samples: Vec<i16> = (0..8192)
        .flat_map(|n| {
            let phase = std::f64::consts::TAU * hz * n as f64 / 48_000.0;
            let v = if hz == 0.0 || hz == 24_000.0 {
                phase.cos() * 15000.0
            } else {
                phase.sin() * 15000.0
            };
            [v as i16, (v * right) as i16]
        })
        .collect();
    analyzer
        .push_i16(&samples, 0, 170_666_667, false)
        .unwrap()
        .unwrap()
}
fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
        .collect()
}

#[test]
fn every_frequency_region_and_independent_stereo_reach_live_graph() {
    for hz in [
        0.0, 46.875, 187.5, 1500.0, 6000.0, 18_000.0, 23_953.125, 24_000.0,
    ] {
        let snapshot = tone(hz, -1.0);
        let mut controller = Controller::new([3840, 2160]).unwrap();
        let output = controller.update(&snapshot, 170_666_667).unwrap();
        assert_eq!(output.buffers["audio_fast"].len(), 513 * 16);
        assert_eq!(output.buffers["audio_detail"].len(), 2049 * 16);
        let fast = floats(&output.buffers["audio_fast"]);
        let bin = (hz / 46.875).round() as usize;
        assert!(
            fast[bin * 4 + 2] > 0.01,
            "missing visible response for {hz}Hz"
        );
        if hz > 0.0 {
            assert!(
                (fast[bin * 4 + 2] - fast[bin * 4 + 3]).abs() < 0.001,
                "antiphase must not cancel"
            );
        }
        assert!(output.parameters["audio_regions"].iter().any(|v| *v > 0.01));
    }
    let snapshot = tone(1500.0, 0.0);
    let output = Controller::new([3840, 2160])
        .unwrap()
        .update(&snapshot, 170_666_667)
        .unwrap();
    let fast = floats(&output.buffers["audio_fast"]);
    assert!(fast[32 * 4 + 2] > 0.1);
    assert_eq!(fast[32 * 4 + 3], 0.0);
}

#[test]
fn fast_updates_do_not_wait_for_detail_and_stale_unavailable_is_explicit() {
    let mut snapshot = tone(93.75, 1.0);
    snapshot.detail_spectrum = None;
    snapshot.tonal_valid = false;
    let mut controller = Controller::new([3840, 2160]).unwrap();
    let output = controller.update(&snapshot, 170_666_667).unwrap();
    assert!(output.parameters["audio_energy"][0] > 0.1);
    assert_eq!(output.parameters["audio_windows"][1], 0.0);
    assert_eq!(output.parameters["audio_space"][2], 0.0);
    let stale = controller.update(&snapshot, 2_170_666_667).unwrap();
    assert!(stale.parameters["audio_energy"][0] < output.parameters["audio_energy"][0] * 0.01);
    assert_eq!(stale.parameters["audio_windows"][0], 0.0);
}

#[test]
fn bass_transient_tonal_changes_and_generation_reset_are_independent() {
    let mut controller = Controller::new([3840, 2160]).unwrap();
    let mut snapshot = tone(93.75, 1.0);
    snapshot.bass_flux = 0.12;
    snapshot.bass_energy = 0.22;
    snapshot.tonal_valid = true;
    snapshot.tonal_confidence = 0.8;
    snapshot.tonal_change = 0.9;
    snapshot.chroma[7] = 1.0;
    let output = controller.update(&snapshot, 170_666_667).unwrap();
    assert!(output.parameters["audio_energy"][1] > 0.0);
    assert!(output.parameters["audio_energy"][2] > 0.0);
    assert!(output.parameters["audio_energy"][3] > 0.0);
    assert!(output.parameters["audio_chroma"][7] > 0.0);
    let empty = Snapshot {
        generation: 2,
        sample_rate: 48_000,
        channels: 2,
        capture_end_ns: 180_666_667,
        ..Default::default()
    };
    let reset = controller.update(&empty, 180_666_667).unwrap();
    assert_eq!(reset.parameters["audio_energy"], [0.0; 4]);
    assert_eq!(reset.parameters["audio_windows"][0], 0.0);
    assert!(reset.buffers["audio_fast"].iter().all(|b| *b == 0));
}

#[test]
fn mismatched_channels_future_analysis_and_regressive_audio_are_rejected() {
    let snapshot = tone(1500.0, 1.0);
    let mut mono = snapshot.clone();
    mono.channels = 1;
    assert!(
        Controller::new([3840, 2160])
            .unwrap()
            .update(&mono, 170_666_667)
            .is_err(),
        "mono cannot advertise right spectral powers"
    );
    let mut future = snapshot.clone();
    future.fast_spectrum.as_mut().unwrap().capture_end_ns += 1;
    assert!(
        Controller::new([3840, 2160])
            .unwrap()
            .update(&future, 170_666_667)
            .is_err(),
        "analysis must not be newer than its source descriptor"
    );
    assert!(
        Controller::new([3840, 2160])
            .unwrap()
            .update(&snapshot, 160_666_667)
            .is_err(),
        "future capture is not causal"
    );
    let mut controller = Controller::new([3840, 2160]).unwrap();
    controller.update(&snapshot, 170_666_667).unwrap();
    let mut regressed = snapshot.clone();
    regressed.end_frame = 4096;
    regressed.fast_spectrum.as_mut().unwrap().end_frame = 4096;
    regressed.detail_spectrum.as_mut().unwrap().end_frame = 4096;
    assert!(
        controller.update(&regressed, 180_666_667).is_err(),
        "an old frame cannot overwrite current generation controls"
    );
}
