use effect_render::scene::Scene;
use effect_runtime::state::FrameInput;

#[test]
fn dense_geometry_preserves_all_authored_state_and_covers_the_full_index_range() {
    let p=milk_import::parser::parse(b"[preset00]\nshapecode_0_enabled=1\nshapecode_0_num_inst=1024\nshapecode_0_sides=3\nshapecode_0_a=1\nshape_0_per_frame1=calls+=1;x=instance/1024;y=.5;rad=.001;\n").unwrap();
    let mut scene = Scene::new(
        milk_compile::bundle::compile_equations(&p).unwrap(),
        [3840, 2160],
        true,
    )
    .unwrap();
    scene.prepare(FrameInput::default(), &[]).unwrap();
    assert_eq!(scene.state.shape_value(0, "calls"), 1024.0);
    assert!(scene.vertices.len() <= 64 * 3 * 3);
    assert!(scene.vertices.iter().any(|v| v.position[0] > 0.95));
}

#[test]
fn shader_volume_tracks_immediate_and_attenuated_bands_independently() {
    let source = milk_import::parser::parse(b"[preset00]\n").unwrap();
    let mut scene = Scene::new(
        milk_compile::bundle::compile_equations(&source).unwrap(),
        [3840, 2160],
        true,
    )
    .unwrap();
    scene
        .prepare(
            FrameInput {
                bass: 3.0,
                mid: 6.0,
                treb: 9.0,
                bass_att: 0.3,
                mid_att: 0.6,
                treb_att: 0.9,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
    assert_eq!(scene.params.data[1][3], 6.0);
    assert!((scene.params.data[2][3] - 0.6).abs() < 1e-6);
}

#[test]
fn source_border_seed_is_emitted_before_the_next_feedback_step() {
    let source = milk_import::parser::parse(b"[preset00]\nfWaveAlpha=0\nob_size=.02\nob_r=1\nob_g=1\nob_b=1\nob_a=1\nper_frame_1=ob_a=1-initialized;initialized=1;\n").unwrap();
    let mut scene = Scene::new(
        milk_compile::bundle::compile_equations(&source).unwrap(),
        [3840, 2160],
        true,
    )
    .unwrap();
    scene.prepare(Default::default(), &[]).unwrap();
    assert!(!scene.vertices.is_empty());
    assert!(scene.vertices.iter().any(|v| v.color[3] == 1.0));
    scene
        .prepare(
            FrameInput {
                frame: 1,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
    assert!(scene.vertices.is_empty());
}

#[test]
fn source_spectrum_wave_reads_frequency_features_instead_of_silent_pcm() {
    let source=milk_import::parser::parse(b"[preset00]\nfWaveAlpha=0\nwavecode_0_enabled=1\nwavecode_0_samples=4\nwavecode_0_bSpectrum=1\nwave_0_per_point1=x=sample;y=.5+value1*.2;\n").unwrap();
    let mut scene = Scene::new(
        milk_compile::bundle::compile_equations(&source).unwrap(),
        [3840, 2160],
        true,
    )
    .unwrap();
    scene.spectrum[32] = 1.0;
    scene.prepare(Default::default(), &[[0.0; 2]; 4]).unwrap();
    assert!(scene.vertices.iter().any(|v| v.position[1] > 0.69));
}

#[test]
fn circular_base_wave_uses_source_position_and_two_dimensions() {
    let source = milk_import::parser::parse(
        b"[preset00]\nnWaveMode=0\nfWaveAlpha=1\nwave_x=.25\nwave_y=.5\n",
    )
    .unwrap();
    let mut scene = Scene::new(
        milk_compile::bundle::compile_equations(&source).unwrap(),
        [3840, 2160],
        true,
    )
    .unwrap();
    scene.prepare(Default::default(), &[[0.0; 2]; 64]).unwrap();
    assert!(scene.vertices.iter().any(|v| v.position[1] > 0.67));
    assert!(scene.vertices.iter().all(|v| v.position[0] < 0.36));
}

#[test]
fn spectrum_base_wave_obeys_source_amplitude_scale() {
    let mut ranges = vec![];
    for scale in [0, 1] {
        let source = milk_import::parser::parse(
            format!("[preset00]\nnWaveMode=8\nfWaveAlpha=1\nfWaveScale={scale}\n").as_bytes(),
        )
        .unwrap();
        let mut scene = Scene::new(
            milk_compile::bundle::compile_equations(&source).unwrap(),
            [3840, 2160],
            true,
        )
        .unwrap();
        scene.spectrum[32] = 1.0;
        scene.prepare(Default::default(), &[[0.0; 2]; 64]).unwrap();
        ranges.push(
            scene
                .vertices
                .iter()
                .map(|v| v.position[1])
                .fold(0.0f32, f32::max),
        );
    }
    assert!(ranges[1] > ranges[0] + 0.14);
}

#[test]
fn shader_pixel_style_changes_sampling_steps_without_changing_canvas() {
    let source = milk_import::parser::parse(b"[preset00]\n").unwrap();
    for adapted in [false, true] {
        let mut scene = Scene::new(
            milk_compile::bundle::compile_equations(&source).unwrap(),
            [3840, 2160],
            adapted,
        )
        .unwrap();
        scene.prepare(Default::default(), &[]).unwrap();
        assert_eq!(scene.canvas, [3840, 2160]);
        assert_eq!(scene.params.data[20][..2], [3840.0, 2160.0]);
        let footprint = if adapted { 3.0 } else { 1.0 };
        assert_eq!(scene.params.data[3][2], footprint / 3840.0);
    }
}
