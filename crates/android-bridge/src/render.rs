use anyhow::{Context, Result, ensure};
use effect_model::shader::PresetPackage;
use effect_render::{
    gpu::{Gpu, Renderer},
    scene::Scene,
};
use effect_runtime::state::FrameInput;
use jni::{
    JNIEnv,
    objects::{JClass, JObject, JString},
    sys::{jint, jlong, jstring},
};
use raw_window_handle::{AndroidDisplayHandle, AndroidNdkWindowHandle};
use std::{
    cell::RefCell, collections::BTreeMap, ffi::c_void, path::Path, ptr::NonNull, time::Instant,
};

#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_fromSurface(env: *mut c_void, surface: *mut c_void) -> *mut c_void;
    fn ANativeWindow_release(window: *mut c_void);
    fn ANativeWindow_setBuffersGeometry(
        window: *mut c_void,
        width: i32,
        height: i32,
        format: i32,
    ) -> i32;
    fn ANativeWindow_getWidth(window: *mut c_void) -> i32;
    fn ANativeWindow_getHeight(window: *mut c_void) -> i32;
}
#[link(name = "GLESv3")]
unsafe extern "C" {
    fn glGetIntegerv(name: u32, value: *mut i32);
    fn glGenFramebuffers(count: i32, value: *mut u32);
    fn glBindFramebuffer(target: u32, name: u32);
    fn glFramebufferTexture2D(
        target: u32,
        attachment: u32,
        texture_target: u32,
        texture: u32,
        level: i32,
    );
    fn glCheckFramebufferStatus(target: u32) -> u32;
    fn glBindBuffer(target: u32, buffer: u32);
    fn glPixelStorei(name: u32, value: i32);
    fn glReadPixels(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        format: u32,
        kind: u32,
        pixels: *mut c_void,
    );
    fn glDeleteFramebuffers(count: i32, value: *const u32);
    fn glGetError() -> u32;
}
struct Window(NonNull<c_void>);
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            ANativeWindow_release(self.0.as_ptr());
        }
    }
}
#[derive(serde::Deserialize)]
struct ComparisonFrame {
    source_sample_end: u64,
    snapshot: audio_analysis::Snapshot,
    waveform: Vec<[f32; 2]>,
}
#[derive(serde::Deserialize)]
struct ComparisonInput {
    frames: Vec<ComparisonFrame>,
}
struct Player {
    capture_pixels: Vec<u8>,
    replay: Option<audio_analysis::replay::ReplayReader>,
    input_offset: u64,
    surface: wgpu::Surface<'static>,
    _window: Window,
    renderer: Renderer,
    scene: Scene,
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    root: std::path::PathBuf,
    rank: usize,
    name: String,
    started: Instant,
    frame: u64,
    last_frame: Option<Instant>,
    deadline_misses: u64,
    attenuated: [f64; 3],
    audio_generation: Option<u64>,
    comparison: Vec<ComparisonFrame>,
}
// All renderer ownership stays on the Java render thread; no raw native window
// crosses thread boundaries. Surface is declared before Window and drops first.
thread_local! {static PLAYERS:RefCell<BTreeMap<i64,Player>>=const {RefCell::new(BTreeMap::new())};static LAST_ERROR:RefCell<String>=const {RefCell::new(String::new())};}

impl Player {
    fn new(
        env: &JNIEnv,
        surface: &JObject,
        root: &Path,
        rank: usize,
        gl_backend: bool,
    ) -> Result<Self> {
        let pointer = unsafe {
            ANativeWindow_fromSurface(env.get_native_interface().cast(), surface.as_raw().cast())
        };
        let window = Window(NonNull::new(pointer).context("Android native surface unavailable")?);
        ensure!(
            unsafe { ANativeWindow_setBuffersGeometry(pointer, 3840, 2160, 0) } == 0,
            "native 4K buffer negotiation failed"
        );
        ensure!(
            unsafe { ANativeWindow_getWidth(pointer) } == 3840
                && unsafe { ANativeWindow_getHeight(pointer) } == 2160,
            "native window is not 3840x2160"
        );
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = if gl_backend {
            wgpu::Backends::GL
        } else {
            wgpu::Backends::VULKAN
        };
        let instance = wgpu::Instance::new(descriptor);
        // SAFETY: acquired ANativeWindow is owned by Window and outlives Surface.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(AndroidDisplayHandle::new().into()),
                raw_window_handle: AndroidNdkWindowHandle::new(window.0).into(),
            })
        }?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            ..Default::default()
        }))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("android-native-4k"),
                required_limits: adapter.limits(),
                ..Default::default()
            }))?;
        let caps = surface.get_capabilities(&adapter);
        let mut config = surface
            .get_default_config(&adapter, 3840, 2160)
            .context("native presentation unsupported")?;
        config.format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 2;
        surface.configure(&device, &config);
        let mut renderer = Renderer::new(
            Gpu {
                device,
                queue,
                adapter,
            },
            3840,
            2160,
        )?;
        let (package, assets) = load(root, rank)?;
        let name = package.source_name.clone();
        let wrap = package
            .equations
            .static_values
            .get("bTexWrap")
            .copied()
            .unwrap_or(1.0)
            != 0.0;
        let scene = Scene::new(package.equations, [3840, 2160], true)?;
        renderer.set_shaders(package.warp, package.comp, assets, wrap)?;
        let device = &renderer.gpu.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("native-4k-presentation"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../effect-render/src/present.wgsl").into(),
            ),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("native-present"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("native-one-to-one-present"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("native-output-present"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(renderer.output_view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let comparison_path = root.join("comparison.json");
        let comparison = if comparison_path.exists() {
            let data: ComparisonInput = serde_json::from_slice(&std::fs::read(comparison_path)?)?;
            ensure!(
                data.frames.len() == 480 && data.frames.iter().all(|f| f.waveform.len() == 512),
                "invalid common PCM comparison frame bounds"
            );
            data.frames
        } else {
            vec![]
        };
        let replay_path = root.join("playlist.frames");
        let replay = if replay_path.exists() {
            Some(audio_analysis::replay::ReplayReader::open(&replay_path)?)
        } else {
            None
        };
        Ok(Self {
            capture_pixels: vec![],
            replay,
            input_offset: 0,
            comparison,
            surface,
            _window: window,
            renderer,
            scene,
            pipeline,
            group,
            root: root.to_owned(),
            rank,
            name,
            started: Instant::now(),
            frame: 0,
            last_frame: None,
            deadline_misses: 0,
            attenuated: [0.0; 3],
            audio_generation: None,
        })
    }
    fn movie_pixels(&mut self) -> Result<()> {
        if self.renderer.gpu.adapter.get_info().backend != wgpu::Backend::Gl {
            self.capture_pixels = self.renderer.capture()?;
            return Ok(());
        }
        // Backend-specific comparison readback through declared wgpu HAL handles.
        // The locked context and texture guards retain ownership; no resource is destroyed.
        let device = unsafe { self.renderer.gpu.device.as_hal::<wgpu::hal::api::Gles>() }
            .context("GLES device handle unavailable")?;
        let texture = unsafe { self.renderer.output.as_hal::<wgpu::hal::api::Gles>() }
            .context("GLES output handle unavailable")?;
        let raw = match texture.inner {
            wgpu::hal::gles::TextureInner::Texture { raw, .. } => raw.0.get(),
            _ => return Err(anyhow::anyhow!("GLES output is not a texture")),
        };
        let _context = device.context().lock();
        self.capture_pixels.resize(3840 * 2160 * 4, 0);
        // Only this render thread accesses these GL objects. Save/restore all changed state.
        unsafe {
            let keys = [0x8CAA, 0x88ED, 0x0D05, 0x0D02, 0x0D03, 0x0D04];
            let mut saved = [0i32; 6];
            for (key, value) in keys.iter().zip(saved.iter_mut()) {
                glGetIntegerv(*key, value);
            }
            let mut fbo = 0;
            glGenFramebuffers(1, &mut fbo);
            glBindFramebuffer(0x8CA8, fbo);
            glFramebufferTexture2D(0x8CA8, 0x8CE0, 0x0DE1, raw, 0);
            let complete = glCheckFramebufferStatus(0x8CA8) == 0x8CD5;
            glBindBuffer(0x88EB, 0);
            for key in &keys[2..] {
                glPixelStorei(*key, if *key == 0x0D05 { 1 } else { 0 });
            }
            for _ in 0..32 {
                if glGetError() == 0 {
                    break;
                }
            }
            if complete {
                glReadPixels(
                    0,
                    0,
                    3840,
                    2160,
                    0x1908,
                    0x1401,
                    self.capture_pixels.as_mut_ptr().cast(),
                );
            }
            let error = glGetError();
            glBindFramebuffer(0x8CA8, saved[0] as u32);
            glBindBuffer(0x88EB, saved[1] as u32);
            for (key, value) in keys[2..].iter().zip(&saved[2..]) {
                glPixelStorei(*key, *value);
            }
            glDeleteFramebuffers(1, &fbo);
            ensure!(
                complete && error == 0,
                "direct GLES comparison readback failed: {error}"
            );
        }
        Ok(())
    }
    fn switch(&mut self, rank: usize) -> Result<()> {
        let (package, assets) = load(&self.root, rank)?;
        let wrap = package
            .equations
            .static_values
            .get("bTexWrap")
            .copied()
            .unwrap_or(1.0)
            != 0.0;
        let scene = Scene::new(package.equations, [3840, 2160], true)?;
        self.renderer
            .set_shaders(package.warp, package.comp, assets, wrap)?;
        self.scene = scene;
        self.name = package.source_name;
        self.rank = rank;
        self.started = Instant::now();
        self.frame = 0;
        self.last_frame = None;
        self.deadline_misses = 0;
        self.attenuated = [0.0; 3];
        self.audio_generation = None;
        Ok(())
    }
    fn draw(&mut self, mode: i32, now_ns: u64) -> Result<String> {
        ensure!((0..=3).contains(&mode), "unsupported input mode");
        ensure!(
            mode != 2 || self.comparison.len() == 480,
            "prepared common PCM input unavailable"
        );
        let replay_frame = if mode == 3 {
            let reader = self
                .replay
                .as_mut()
                .context("playlist replay unavailable")?;
            let f = reader.read(
                self.input_offset
                    .checked_add(self.frame)
                    .context("playlist frame overflow")?,
            )?;
            Some(ComparisonFrame {
                source_sample_end: f.source_end,
                snapshot: f.snapshot(reader.rate),
                waveform: f.waveform(),
            })
        } else {
            None
        };
        let fixture = mode == 1;
        let before = Instant::now();
        let interval = self
            .last_frame
            .map(|t| before.duration_since(t).as_secs_f64() * 1000.0);
        if interval.is_some_and(|ms| ms > 36.0) {
            self.deadline_misses += 1;
        }
        self.last_frame = Some(before);
        let time = if mode >= 2 {
            (self.frame + 1) as f64 / 30.0
        } else {
            self.started.elapsed().as_secs_f64()
        };
        let mut waveform = [[0.0f32; 2]; 512];
        if fixture {
            for (i, v) in waveform.iter_mut().enumerate() {
                let t = time + i as f64 / 48000.0;
                *v = [
                    ((t * std::f64::consts::TAU * 110.0).sin()
                        * (0.12 + 0.4 * (-(t % 0.5) * 20.0).exp())) as f32,
                    (t * std::f64::consts::TAU
                        * 220.0
                        * 2.0f64.powf(([0.0, 4.0, 7.0, 12.0][time as usize % 4]) / 12.0))
                    .sin() as f32
                        * 0.25,
                ];
            }
        }
        let report = super::android::latest_report();
        let comparison = if let Some(f) = replay_frame.as_ref() {
            Some(f)
        } else if mode == 2 {
            Some(&self.comparison[self.frame as usize % 480])
        } else {
            None
        };
        let snapshot = comparison.map(|f| &f.snapshot).or_else(|| {
            report
                .as_ref()
                .and_then(|r| r.snapshot.as_ref())
                .filter(|s| now_ns.saturating_sub(s.capture_end_ns) < 150_000_000)
        });
        let mut bass = 0.0;
        let mut mid = 0.0;
        let mut treb = 0.0;
        let mut source = "waiting-for-playback-PCM";
        if let Some(s) = snapshot {
            source = if comparison.is_some() {
                if mode == 3 {
                    "Beatport-lossless-source-stereo-PCM-replay"
                } else {
                    "controlled-common-PCM-replay-48k-stereo"
                }
            } else {
                "continuous-playback-PCM"
            };
            bass = ((s.bass_energy + s.sub_bass_energy).sqrt() * 8.0).min(4.0);
            for (i, p) in s.bands.iter().enumerate() {
                let hz = 20.0 * (1000.0f64).powf((i as f64 + 0.5) / 64.0);
                if (250.0..4000.0).contains(&hz) {
                    mid += p;
                } else if hz >= 4000.0 {
                    treb += p;
                }
            }
            mid = (mid.sqrt() * 8.0).min(4.0);
            treb = (treb.sqrt() * 8.0).min(4.0);
            if let Some(input) = comparison {
                waveform.copy_from_slice(&input.waveform);
            } else if let Some(r) = &report {
                for (to, from) in waveform.iter_mut().zip(&r.waveform) {
                    *to = *from;
                }
            }
        } else if fixture {
            source = "explicit-synthetic-stereo-control";
            let pulse = (-(time % 0.5) * 20.0).exp();
            bass = 0.3 + 2.5 * pulse;
            mid = 0.7 + 0.4 * (time * 1.7).sin().abs();
            treb = 0.5 + 1.2 * pulse;
        }
        let generation = snapshot.map(|s| s.generation);
        if generation != self.audio_generation {
            self.attenuated = [0.0; 3];
            self.audio_generation = generation;
        }
        let dt = if mode >= 2 {
            1.0 / 30.0
        } else {
            interval.unwrap_or(1000.0 / 60.0).min(100.0) / 1000.0
        };
        for (envelope, level) in self.attenuated.iter_mut().zip([bass, mid, treb]) {
            let seconds = if level > *envelope { 0.04 } else { 0.25 };
            *envelope += (level - *envelope) * (1.0 - (-dt / seconds).exp());
        }
        self.scene.spectrum.fill(0.0);
        if let Some(s) = snapshot {
            for (to, energy) in self.scene.spectrum.iter_mut().zip(&s.bands) {
                *to = (energy.sqrt() * 8.0).min(4.0) as f32;
            }
        } else if fixture {
            self.scene.spectrum[12] = bass as f32 * 0.5;
            self.scene.spectrum[28 + time as usize % 4 * 3] = mid as f32 * 0.5;
        }
        let prepare_start = Instant::now();
        self.scene.prepare(
            FrameInput {
                frame: self.frame,
                fps: if mode >= 2 { 30.0 } else { 60.0 },
                time,
                progress: (time / 30.0).min(1.0),
                bass,
                mid,
                treb,
                bass_att: self.attenuated[0],
                mid_att: self.attenuated[1],
                treb_att: self.attenuated[2],
                ..FrameInput::default()
            },
            &waveform,
        )?;
        let prepare_ms = prepare_start.elapsed().as_secs_f64() * 1000.0;
        let submit_start = Instant::now();
        self.renderer.render(
            &self.scene.params,
            &self.scene.mesh,
            &self.scene.vertices,
            &self.scene.batches,
        )?;
        let submit_ms = submit_start.elapsed().as_secs_f64() * 1000.0;
        let acquire_start = Instant::now();
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => return Err(anyhow::anyhow!("native surface acquire skipped/lost")),
        };
        let acquire_ms = acquire_start.elapsed().as_secs_f64() * 1000.0;
        ensure!(
            output.texture.width() == 3840 && output.texture.height() == 2160,
            "presentation surface lost native 4K size"
        );
        let view = output.texture.create_view(&Default::default());
        let mut encoder = self
            .renderer
            .gpu
            .device
            .create_command_encoder(&Default::default());
        {
            let colors = [Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("native-4k-screen"),
                color_attachments: &colors,
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.renderer.gpu.queue.submit([encoder.finish()]);
        self.renderer.gpu.queue.present(output);
        self.frame += 1;
        Ok(serde_json::json!({"state":"rendering","mode":if self.scene.adapted {"adapted-4k"}else{"source-stroke-comparison"},"preset":self.name,"rank":self.rank,"frame":self.frame,"frame_start_ns":now_ns,"target_fps":if mode>=2{30}else{60},"surface":[3840,2160],"native_allocations":self.renderer.allocation_dimensions(),"cpu_frame_ms":before.elapsed().as_secs_f64()*1000.0,"prepare_ms":prepare_ms,"geometry_vertices":self.scene.vertices.len(),"vm_instructions":self.scene.state.instruction_count(),"submit_ms":submit_ms,"acquire_ms":acquire_ms,"frame_interval_ms":interval,"deadline_misses":self.deadline_misses,"input":source,"feature_age_ms":snapshot.filter(|_|mode<2).map(|s|now_ns.saturating_sub(s.capture_end_ns)as f64/1e6),"audio_generation":snapshot.map(|s|s.generation),"comparison_analysis_lag_samples":comparison.map(|f|f.source_sample_end.saturating_sub(f.snapshot.end_frame)),"recording":mode==3,"playback_benchmark_eligible":mode!=3,"input_source_frame":self.input_offset+self.frame-1,"resource_substitutions":self.renderer.resource_substitutions(),"timing_scope":"CPU prepare/submit/acquire/present-call; no GPU completion claim","diagnostic_interval_boundary_ms":36.0,"graphics_backend":format!("{:?}",self.renderer.gpu.adapter.get_info().backend),"adapter":self.renderer.gpu.adapter.get_info().name}).to_string())
    }
}
fn load(root: &Path, rank: usize) -> Result<(PresetPackage, BTreeMap<String, Vec<u8>>)> {
    ensure!((1..=100).contains(&rank), "preset rank outside frozen 100");
    let package: PresetPackage =
        serde_json::from_slice(&std::fs::read(root.join(format!("{rank:03}.json")))?)?;
    let mut assets = BTreeMap::new();
    for entry in std::fs::read_dir(root.join("textures"))? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            assets.insert(
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path())?,
            );
        }
    }
    Ok((package, assets))
}
fn error(message: String) {
    LAST_ERROR.with(|s| *s.borrow_mut() = message);
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeRender_create(
    mut env: JNIEnv,
    _class: JClass,
    surface: JObject,
    directory: JString,
    rank: jint,
    backend: jint,
) -> jlong {
    let result = (|| -> Result<Player> {
        let path: String = env.get_string(&directory)?.into();
        Player::new(
            &env,
            &surface,
            Path::new(&path),
            rank as usize,
            backend == 1,
        )
    })();
    match result {
        Ok(player) => {
            PLAYERS.with(|p| p.borrow_mut().insert(1, player));
            1
        }
        Err(e) => {
            error(format!("{e:#}"));
            0
        }
    }
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeRender_frame(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    rank: jint,
    fixture: jint,
    adapted: jint,
    now_ns: jlong,
) -> jstring {
    let result = PLAYERS.with(|p| -> Result<String> {
        let mut p = p.borrow_mut();
        let player = p
            .get_mut(&handle)
            .context("renderer handle unavailable on render thread")?;
        if player.rank != rank as usize || player.scene.adapted != (adapted != 0) {
            player.switch(rank as usize)?;
            player.scene.adapted = adapted != 0;
        }
        player.draw(fixture, now_ns as u64)
    });
    let json = match result {
        Ok(j) => j,
        Err(e) => serde_json::json!({"state":"failed","error":format!("{e:#}")}).to_string(),
    };
    env.new_string(json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeRender_error(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let message = LAST_ERROR.with(|s| s.borrow().clone());
    env.new_string(message)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeRender_destroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    PLAYERS.with(|p| p.borrow_mut().remove(&handle));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeRender_capture(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    path: JString,
) -> jstring {
    let result = (|| -> Result<String> {
        let path: String = env.get_string(&path)?.into();
        PLAYERS.with(|p|->Result<String>{let p=p.borrow();let player=p.get(&handle).context("capture renderer unavailable")?;let rgba=player.renderer.capture()?;let image=image::RgbaImage::from_raw(3840,2160,rgba).context("native capture size mismatch")?;let file=std::fs::File::create(&path)?;image::codecs::jpeg::JpegEncoder::new_with_quality(file,92).encode_image(&image::DynamicImage::ImageRgba8(image))?;Ok(serde_json::json!({"rank":player.rank,"source":player.name,"path":path,"dimensions":[3840,2160],"capture_backend":"wgpu native RGBA8 readback, JPEG preview only","performance_sample":false}).to_string())})
    })();
    let result = match result {
        Ok(s) => s,
        Err(e) => serde_json::json!({"error":format!("{e:#}")}).to_string(),
    };
    env.new_string(result)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeRender_inputOffset(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    offset: jlong,
) {
    PLAYERS.with(|p| {
        if let Some(player) = p.borrow_mut().get_mut(&handle) {
            player.input_offset = offset.max(0) as u64;
        }
    });
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeRender_pixels(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jni::sys::jobject {
    let result = PLAYERS.with(|p| -> Result<jni::objects::JByteBuffer> {
        let mut p = p.borrow_mut();
        let player = p.get_mut(&handle).context("capture renderer unavailable")?;
        player.movie_pixels()?;
        // SAFETY: Java copies this buffer synchronously on the render thread.
        // Its native storage remains owned by Player until the next capture/drop.
        Ok(unsafe {
            env.new_direct_byte_buffer(
                player.capture_pixels.as_mut_ptr(),
                player.capture_pixels.len(),
            )
        }?)
    });
    match result {
        Ok(b) => b.into_raw(),
        Err(e) => {
            error(format!("{e:#}"));
            std::ptr::null_mut()
        }
    }
}
