use effect_render::{
    geometry::stroke,
    gpu::{Gpu, MeshVertex, Params, Renderer},
};

#[test]
fn native_history_and_output_render_a_three_pixel_stroke_at_4k() {
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    let mut renderer = Renderer::new(gpu, 3840, 2160).unwrap();
    assert_eq!(renderer.output.width(), 3840);
    assert_eq!(renderer.output.height(), 2160);
    assert_eq!(renderer.allocation_dimensions(), [[3840, 2160]; 3]);
    let mesh: Vec<_> = (0..=32)
        .flat_map(|y| {
            (0..=48).map(move |x| {
                let uv = [x as f32 / 48.0, y as f32 / 32.0];
                MeshVertex {
                    position: uv,
                    original: uv,
                    warped: uv,
                }
            })
        })
        .collect();
    let mut params = Params::default();
    params.data[7][3] = 0.98;
    params.data[18] = [1.0, 1.0, 0.0, 0.0];
    let vertices = stroke([0.1, 0.5], [0.9, 0.5], [1.0; 4], 3.0, [3840, 2160]);
    renderer
        .render(
            &params,
            &mesh,
            &vertices,
            &[(0..vertices.len() as u32, false)],
        )
        .unwrap();
    let pixels = renderer.capture().unwrap();
    assert_eq!(pixels.len(), 3840 * 2160 * 4);
    let lit_rows: Vec<_> = (0..2160)
        .filter(|y| pixels[(y * 3840 + 1920) * 4] > 100)
        .collect();
    assert_eq!(lit_rows.len(), 3);
}

#[test]
fn authored_fragment_output_replaces_the_fixed_composite() {
    let gpu = pollster::block_on(Gpu::headless()).unwrap();
    let mut renderer = Renderer::new(gpu, 3840, 2160).unwrap();
    let shader=effect_model::shader::Shader{wgsl:"@fragment fn main(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{return vec4<f32>(uv,0.0,1.0);}".into(),bindings:vec![],entry:"main".into(),texture_sizes:vec![],loop_budget:Some(64)};
    renderer
        .set_shaders(None, Some(shader), Default::default(), true)
        .unwrap();
    let mesh: Vec<_> = (0..=32)
        .flat_map(|y| {
            (0..=48).map(move |x| {
                let uv = [x as f32 / 48.0, y as f32 / 32.0];
                MeshVertex {
                    position: uv,
                    original: uv,
                    warped: uv,
                }
            })
        })
        .collect();
    renderer
        .render(&Params::default(), &mesh, &[], &[])
        .unwrap();
    let pixels = renderer.capture().unwrap();
    assert!((pixels[(1080 * 3840 + 1920) * 4] as i32 - 128).abs() < 2);
    assert!(pixels[(1080 * 3840 + 3400) * 4] > 220);
}

#[test]
fn bounded_authored_shader_samples_native_feedback_and_blur() {
    let source = milk_compile::shader::from_wgsl(
        r#"
@group(0) @binding(0) var sampler_fc_main_0: texture_2d<f32>;
@group(0) @binding(1) var _sampler_fc_main_sampler_0: sampler;
@group(0) @binding(2) var sampler_blur2_0: texture_2d<f32>;
@group(0) @binding(3) var _sampler_blur2_sampler_0: sampler;
@fragment fn main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
return 0.5 * (textureSample(sampler_fc_main_0, _sampler_fc_main_sampler_0, uv)
             + textureSample(sampler_blur2_0, _sampler_blur2_sampler_0, uv));
}"#,
    )
    .unwrap();
    let shader = milk_compile::shader::bounded_wgsl(&source.wgsl, 64).unwrap();
    let mut renderer =
        Renderer::new(pollster::block_on(Gpu::headless()).unwrap(), 128, 128).unwrap();
    // No image assets: a mistaken built-in substitution must fail, not look plausible.
    renderer
        .set_shaders(None, Some(shader), Default::default(), false)
        .unwrap();
    let mesh = mesh_at(None);
    let mut params = Params::default();
    params.data[7][3] = 1.0;
    let vertices = stroke(
        [0.0, 0.5],
        [1.0, 0.5],
        [1.0, 0.0, 0.0, 1.0],
        128.0,
        [128, 128],
    );
    renderer
        .render(
            &params,
            &mesh,
            &vertices,
            &[(0..vertices.len() as u32, false)],
        )
        .unwrap();
    let pixels = renderer.capture().unwrap();
    assert!(pixels[(64 * 128 + 64) * 4] > 240);
    assert_eq!(pixels[(64 * 128 + 64) * 4 + 2], 0);
}

fn mesh_at(sample: Option<[f32; 2]>) -> Vec<MeshVertex> {
    (0..=32)
        .flat_map(|y| {
            (0..=48).map(move |x| {
                let uv = [x as f32 / 48.0, y as f32 / 32.0];
                MeshVertex {
                    position: uv,
                    original: uv,
                    warped: sample.unwrap_or(uv),
                }
            })
        })
        .collect()
}

#[test]
fn fixed_feedback_obeys_declared_clamp_and_repeat() {
    for wrap in [false, true] {
        let mut renderer =
            Renderer::new(pollster::block_on(Gpu::headless()).unwrap(), 128, 128).unwrap();
        renderer
            .set_shaders(None, None, Default::default(), wrap)
            .unwrap();
        let mut params = Params::default();
        params.data[7][3] = 1.0;
        params.data[18] = [1.0, 1.0, 0.0, 0.0];
        let mut vertices = stroke(
            [0.0, 0.5],
            [0.5, 0.5],
            [1.0, 0.0, 0.0, 1.0],
            128.0,
            [128, 128],
        );
        vertices.extend(stroke(
            [0.5, 0.5],
            [1.0, 0.5],
            [0.0, 0.0, 1.0, 1.0],
            128.0,
            [128, 128],
        ));
        renderer
            .render(
                &params,
                &mesh_at(None),
                &vertices,
                &[(0..vertices.len() as u32, false)],
            )
            .unwrap();
        renderer
            .render(&params, &mesh_at(Some([-0.1, 0.5])), &[], &[])
            .unwrap();
        let pixels = renderer.capture().unwrap();
        let center = (64 * 128 + 64) * 4;
        assert!(
            pixels[center + if wrap { 2 } else { 0 }] > 240,
            "wrap={wrap}: {:?}",
            &pixels[center..center + 4]
        );
    }
}

#[test]
fn textured_shapes_preserve_feedback_instead_of_emitting_flat_white() {
    let source = milk_import::parser::parse(b"[preset00]\nfWaveAlpha=0\nfDecay=1\nfGammaAdj=1\nshapecode_0_enabled=1\nshapecode_0_textured=1\nshapecode_0_sides=4\nshapecode_0_rad=2\nshapecode_0_x=.5\nshapecode_0_y=.5\nshapecode_0_r=1\nshapecode_0_g=1\nshapecode_0_b=1\nshapecode_0_a=1\nshapecode_0_r2=1\nshapecode_0_g2=1\nshapecode_0_b2=1\nshapecode_0_a2=1\nshapecode_0_tex_zoom=1\n").unwrap();
    let mut scene = effect_render::scene::Scene::new(
        milk_compile::bundle::compile_equations(&source).unwrap(),
        [128, 128],
        true,
    )
    .unwrap();
    scene.prepare(Default::default(), &[]).unwrap();
    let mut renderer =
        Renderer::new(pollster::block_on(Gpu::headless()).unwrap(), 128, 128).unwrap();
    renderer
        .render(&scene.params, &scene.mesh, &scene.vertices, &scene.batches)
        .unwrap();
    let pixels = renderer.capture().unwrap();
    assert_eq!(
        &pixels[(64 * 128 + 64) * 4..(64 * 128 + 64) * 4 + 3],
        &[0, 0, 0]
    );
    let red = stroke(
        [0.0, 0.5],
        [1.0, 0.5],
        [1.0, 0.0, 0.0, 1.0],
        128.0,
        [128, 128],
    );
    renderer
        .render(
            &scene.params,
            &scene.mesh,
            &red,
            &[(0..red.len() as u32, false)],
        )
        .unwrap();
    renderer
        .render(&scene.params, &scene.mesh, &scene.vertices, &scene.batches)
        .unwrap();
    let pixels = renderer.capture().unwrap();
    assert_eq!(
        &pixels[(64 * 128 + 64) * 4..(64 * 128 + 64) * 4 + 3],
        &[255, 0, 0]
    );
}

#[test]
fn exact_origin_seed_runs_as_native_three_pixel_cells_in_adapted_mode() {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let corpus = milk_import::corpus::verify(root).unwrap();
    for adapted in [false, true] {
        let package = milk_compile::package::from_artifacts(root, &corpus.presets[60]).unwrap();
        let mut scene =
            effect_render::scene::Scene::new(package.equations, [128, 128], adapted).unwrap();
        let mut renderer =
            Renderer::new(pollster::block_on(Gpu::headless()).unwrap(), 128, 128).unwrap();
        renderer
            .set_shaders(package.warp, package.comp, Default::default(), false)
            .unwrap();
        for frame in 0..8 {
            scene
                .prepare(
                    effect_runtime::state::FrameInput {
                        frame,
                        ..Default::default()
                    },
                    &[],
                )
                .unwrap();
            renderer
                .render(&scene.params, &scene.mesh, &scene.vertices, &scene.batches)
                .unwrap();
        }
        let pixels = renderer.capture().unwrap();
        assert_eq!(pixels.chunks_exact(4).any(|p| p[0] > 200), adapted);
        if adapted {
            assert!(pixels.chunks_exact(4).filter(|p| p[0] > 200).count() >= 9);
        }
    }
}
