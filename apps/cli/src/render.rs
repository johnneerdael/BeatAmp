use anyhow::{Context, Result, ensure};
use effect_render::{
    gpu::{Gpu, Renderer},
    scene::Scene,
};
use effect_runtime::state::FrameInput;
use milk_import::{corpus, parser};
use std::{io::Cursor, path::Path};

pub(crate) fn fixed(root: &Path, rank: u32, frames: u32, output: &Path) -> Result<()> {
    ensure!(
        (1..=100).contains(&rank) && frames > 0 && frames <= 480,
        "invalid corpus rank/frame count"
    );
    let corpus = corpus::verify(root)?;
    let entry = &corpus.presets[rank as usize - 1];
    let source = parser::parse(&corpus::verify_entry(root, entry, "presets")?)?;
    ensure!(
        !source.sections.contains_key("warp") && !source.sections.contains_key("comp"),
        "preset has authored HLSL; fixed-function rendering cannot substitute for its shader"
    );
    let mut scene = Scene::new(
        milk_compile::bundle::compile_equations(&source)?,
        [3840, 2160],
        true,
    )?;
    let gpu = pollster::block_on(Gpu::headless())?;
    let adapter = gpu.adapter.get_info();
    let mut renderer = Renderer::new(gpu, 3840, 2160)?;
    let mut waveform = vec![[0.0f32; 2]; 512];
    for frame in 0..frames {
        let time = frame as f64 / 30.0;
        // Explicit continuous stereo synthetic control; not claimed as captured music.
        for (i, value) in waveform.iter_mut().enumerate() {
            let t = time + i as f64 / 48000.0;
            *value = [
                (t * std::f64::consts::TAU * 110.0).sin() as f32 * 0.35,
                (t * std::f64::consts::TAU * 440.0).sin() as f32 * 0.25,
            ];
        }
        let input = FrameInput {
            frame: frame as u64,
            time,
            progress: frame as f64 / 480.0,
            bass: 1.0 + (time * std::f64::consts::TAU * 2.0).sin() * 0.4,
            ..FrameInput::default()
        };
        scene.prepare(input, &waveform)?;
        renderer.render(&scene.params, &scene.mesh, &scene.vertices, &scene.batches)?;
    }
    let rgba = renderer.capture()?;
    let image =
        image::RgbaImage::from_raw(3840, 2160, rgba).context("native image dimensions mismatch")?;
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image).write_to(&mut png, image::ImageFormat::Png)?;
    let output = if output.is_absolute() {
        output.to_owned()
    } else {
        root.join(output)
    };
    super::write_bytes(root, &output, png.get_ref())?;
    let metadata = serde_json::json!({"source_id":entry.id,"source_name":entry.path,"source_sha256":entry.sha256,"producer":"wgpu-fixed-function-vertical-slice-v2","preset_rank":rank,"adapter":format!("{adapter:?}"),"feedback":[3840,2160],"output":[3840,2160],"primary_format":"RGBA8Unorm","minimum_stroke_native_pixels":3,"stroke_policy":"native-coverage-density-v1: line alpha/width; point alpha/area; clamp alpha to native RGBA8 domain before coverage adjustment","input":"synthetic stereo 48kHz controls","logical_fps":30,"frames":frames,"capture_frame":frames-1,"mode":"adapted","scope":"source equations/custom waves/shapes; base wave and warp kernels preliminary; no authored HLSL replacement","renderer_qualified_4k30":false,"image_sha256":corpus::sha256(png.get_ref())});
    super::write_report(
        root,
        &output.with_extension("json"),
        &serde_json::to_string_pretty(&metadata)?,
    )?;
    println!(
        "Rendered {} at native 3840x2160: {}",
        entry.path,
        output.display()
    );
    Ok(())
}

pub(crate) fn packages(root: &Path) -> Result<()> {
    let corpus = corpus::verify(root)?;
    let mut catalog = vec![];
    for entry in &corpus.presets {
        let package = milk_compile::package::from_artifacts(root, entry)?;
        let json = serde_json::to_vec(&package)?;
        let digest = corpus::sha256(&json);
        let relative = format!("effects/{}/{digest}/preset.json", package.source_id);
        super::write_bytes(root, &root.join(&relative), &json)?;
        catalog.push(serde_json::json!({"rank":entry.selection_rank,"name":package.source_name,"source_id":package.source_id,"source_sha256":package.source_sha256,"package_sha256":digest,"path":relative}));
    }
    super::write_report(
        root,
        &root.join("reports/poc-catalog.json"),
        &serde_json::to_string_pretty(
            &serde_json::json!({"schema":1,"count":100,"presets":catalog}),
        )?,
    )?;
    println!(
        "Published 100 procedural package artifacts; render/visual/performance checks remain separate."
    );
    Ok(())
}
pub(crate) fn poc(root: &Path, rank: u32, frames: u32, output: &Path) -> Result<()> {
    ensure!(
        (1..=100).contains(&rank) && frames > 0 && frames <= 480,
        "invalid corpus rank/frame count"
    );
    let corpus = corpus::verify(root)?;
    let entry = &corpus.presets[rank as usize - 1];
    let package = milk_compile::package::from_artifacts(root, entry)?;
    let mut assets = std::collections::BTreeMap::new();
    for texture in &corpus.textures {
        assets.insert(
            texture.path.clone(),
            corpus::verify_entry(root, texture, "textures")?,
        );
    }
    let main_wrap = package
        .equations
        .static_values
        .get("bTexWrap")
        .copied()
        .unwrap_or(1.0)
        != 0.0;
    let mut scene = Scene::new(package.equations, [3840, 2160], true)?;
    let gpu = pollster::block_on(Gpu::headless())?;
    let info = gpu.adapter.get_info();
    let mut renderer = Renderer::new(gpu, 3840, 2160)?;
    renderer.set_shaders(package.warp, package.comp, assets, main_wrap)?;
    let mut waveform = vec![[0.0f32; 2]; 512];
    for frame in 0..frames {
        let time = frame as f64 / 30.0;
        for (i, v) in waveform.iter_mut().enumerate() {
            let t = time + i as f64 / 48000.0;
            *v = [
                (t * std::f64::consts::TAU * 110.0).sin() as f32 * 0.35,
                (t * std::f64::consts::TAU * 440.0).sin() as f32 * 0.25,
            ];
        }
        scene.prepare(
            FrameInput {
                frame: frame as u64,
                time,
                progress: frame as f64 / 480.0,
                bass: 1.0 + (time * std::f64::consts::TAU * 2.0).sin() * 0.4,
                ..FrameInput::default()
            },
            &waveform,
        )?;
        renderer.render(&scene.params, &scene.mesh, &scene.vertices, &scene.batches)?;
    }
    let rgba = renderer.capture()?;
    let image =
        image::RgbaImage::from_raw(3840, 2160, rgba).context("native output size mismatch")?;
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image).write_to(&mut png, image::ImageFormat::Png)?;
    let output = if output.is_absolute() {
        output.to_owned()
    } else {
        root.join(output)
    };
    super::write_bytes(root, &output, png.get_ref())?;
    super::write_report(
        root,
        &output.with_extension("json"),
        &serde_json::to_string_pretty(
            &serde_json::json!({"source_name":entry.path,"source_id":entry.id,"source_sha256":entry.sha256,"producer":"authored-native-poc-render-v1","adapter":format!("{info:?}"),"native_allocations":renderer.allocation_dimensions(),"shader_loop_budget":64,"frames":frames,"logical_fps":30,"input":"synthetic stereo control","minimum_stroke_pixels":3,"image_sha256":corpus::sha256(png.get_ref()),"poc_verified_4k30":false}),
        )?,
    )?;
    println!("Rendered authored package {}: {}", rank, output.display());
    Ok(())
}
