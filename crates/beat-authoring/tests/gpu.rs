#![cfg(feature = "offline")]
use audio_analysis::Analyzer;
use beat_authoring::{Controller, SourceHints, SynthesisRequest, synthesize};
use effect_model::beat::Capabilities;
use effect_render::{
    beat::{BeatInput, BeatRenderer},
    gpu::Gpu,
};

#[test]
fn actual_native_gpu_scene_and_feedback_change_with_complete_live_controls() {
    let caps = Capabilities::baseline([3840, 2160]);
    // Exercise both generated graph shapes on the actual adapter.
    for feedback in [false, true] {
        let (source, result) = (0..100)
            .find_map(|i| {
                let source = SourceHints::from_bytes(
                    format!("[preset00]\nfWaveScale={i}\nfDecay=0.91\n").as_bytes(),
                )
                .unwrap();
                let result = synthesize(&SynthesisRequest {
                    name: "Actual generated GPU scene",
                    source: &source,
                    prediction: None,
                    capabilities: &caps,
                })
                .unwrap();
                (result.package.manifest.resources.contains_key("transport") == feedback)
                    .then_some((source, result))
            })
            .unwrap();
        let mut renderer = BeatRenderer::new(
            pollster::block_on(Gpu::headless()).unwrap(),
            result.package,
            [3840, 2160],
        )
        .unwrap();
        let report = renderer.allocation_report();
        assert_eq!(report.dimensions["display"], [[3840, 2160]]);
        if feedback {
            assert_eq!(report.dimensions["transport"], [[3840, 2160]; 2]);
        }
        renderer.render().unwrap();
        let silent = renderer.capture().unwrap();
        let mut analyzer = Analyzer::new(48_000, 2, 1).unwrap();
        let pcm: Vec<i16> = (0..8192)
            .flat_map(|n| {
                let bass = (std::f64::consts::TAU * 93.75 * n as f64 / 48000.0).sin() * 8000.0;
                let high = (std::f64::consts::TAU * 23953.125 * n as f64 / 48000.0).sin() * 8000.0;
                [(bass + high) as i16, (bass - high) as i16]
            })
            .collect();
        let snapshot = analyzer
            .push_i16(&pcm, 0, 170_666_667, false)
            .unwrap()
            .unwrap();
        let frame = Controller::new([3840, 2160])
            .unwrap()
            .update(&snapshot, 170_666_667)
            .unwrap();
        renderer
            .update(&BeatInput {
                parameters: frame.parameters,
                buffers: frame.buffers,
                ..Default::default()
            })
            .unwrap();
        renderer.render().unwrap();
        let reactive = renderer.capture().unwrap();
        let changed = silent
            .chunks_exact(4)
            .zip(reactive.chunks_exact(4))
            .filter(|(a, b)| a[..3].iter().zip(&b[..3]).any(|(x, y)| x.abs_diff(*y) > 3))
            .count();
        assert!(
            changed > 3840 * 2160 / 10,
            "source{} feedback{feedback} only changed{changed}pixels",
            source.sha256
        );
        assert!(
            reactive
                .chunks_exact(4)
                .any(|v| v[0] > 80 || v[1] > 80 || v[2] > 80)
        );
    }
}
