//! Independent .beat Android player; source parsing and prediction stay offline.
use anyhow::{Context, Result, ensure};
use audio_analysis::{Analyzer, Snapshot};
use beat_authoring::Controller;
use effect_package::{Limits, Package};
use effect_render::{
    beat::{BeatAudioDescriptor, BeatInput, BeatRenderer},
    gpu::Gpu,
};
use jni::{
    JNIEnv,
    objects::{JClass, JObject, JString},
    sys::{jint, jlong, jstring},
};
use raw_window_handle::{AndroidDisplayHandle, AndroidNdkWindowHandle};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::c_void,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    ptr::NonNull,
    time::Instant,
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
struct Window(NonNull<c_void>);
#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(
        priority: i32,
        tag: *const std::ffi::c_char,
        text: *const std::ffi::c_char,
    ) -> i32;
}
fn log(message: &str) {
    if let Ok(text) = std::ffi::CString::new(message.replace('\0', "?")) {
        // SAFETY: both strings are NUL-terminated and remain live for the call.
        unsafe {
            __android_log_write(6, c"BeatampNative".as_ptr(), text.as_ptr());
        }
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            ANativeWindow_release(self.0.as_ptr());
        }
    }
}
struct Player {
    surface: wgpu::Surface<'static>,
    _window: Window,
    renderer: BeatRenderer,
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    root: PathBuf,
    rank: usize,
    name: String,
    controller: Controller,
    analyzer: Analyzer,
    last_snapshot: Option<Snapshot>,
    mode: Option<i32>,
    frame: u64,
    input_offset: u64,
    input_file: Option<std::fs::File>,
    clock_start: Option<u64>,
    last_frame: Option<Instant>,
    pixels: Vec<u8>,
}
// Surface drops before the owned native window, always on its Java render thread.
thread_local! { static PLAYERS:RefCell<BTreeMap<i64,Player>>=const {RefCell::new(BTreeMap::new())}; static ERROR:RefCell<String>=const {RefCell::new(String::new())}; }
fn load(root: &Path, rank: usize) -> Result<Package> {
    ensure!((1..=100).contains(&rank), ".beat rank outside frozen100");
    Package::read(
        std::fs::File::open(root.join(format!("{rank:03}.beat")))?,
        &Limits::default(),
    )
}
fn present(
    renderer: &BeatRenderer,
    format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroup) {
    // Sampling sRGB decodes to linear. Encode explicitly only for a linear surface.
    let code = format!(
        r#"
@group(0) @binding(0) var output:texture_2d<f32>;
@vertex fn vertex(@builtin(vertex_index)i:u32)->@builtin(position)vec4<f32>{{
 var p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));return vec4(p[i],0.,1.);
}}
@fragment fn fragment(@builtin(position)p:vec4<f32>)->@location(0)vec4<f32>{{
 let v=textureLoad(output,vec2<i32>(p.xy),0);
 if({}){{let c=max(v.rgb,vec3(0.));let encoded=select(1.055*pow(c,vec3(1./2.4))-0.055,12.92*c,c<=vec3(0.0031308));return vec4(encoded,v.a);}}
 return v;
}}
"#,
        !format.is_srgb()
    );
    let device = &renderer.gpu.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("beatamp-native-present"),
        source: wgpu::ShaderSource::Wgsl(code.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("beatamp-native-present"),
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
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("beatamp-output"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(renderer.output_view()),
        }],
    });
    (pipeline, group)
}
impl Player {
    fn new(env: &JNIEnv, surface: &JObject, root: &Path, rank: usize, gl: bool) -> Result<Self> {
        log("startup: loading standalone package");
        let package = load(root, rank)?;
        let name = package.manifest.name.clone();
        let pointer = unsafe {
            ANativeWindow_fromSurface(env.get_native_interface().cast(), surface.as_raw().cast())
        };
        let window = Window(NonNull::new(pointer).context("Android .beat surface unavailable")?);
        ensure!(
            unsafe { ANativeWindow_setBuffersGeometry(pointer, 3840, 2160, 0) } == 0,
            "native4K surface negotiation failed"
        );
        ensure!(
            unsafe { ANativeWindow_getWidth(pointer) } == 3840
                && unsafe { ANativeWindow_getHeight(pointer) } == 2160,
            ".beat surface is not native4K"
        );
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = if gl {
            wgpu::Backends::GL
        } else {
            wgpu::Backends::VULKAN
        };
        let instance = wgpu::Instance::new(descriptor);
        // SAFETY: Window owns the acquired ANativeWindow until after surface drops.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(AndroidDisplayHandle::new().into()),
                raw_window_handle: AndroidNdkWindowHandle::new(window.0).into(),
            })
        }?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("beatamp-native4K"),
                required_limits: adapter.limits(),
                ..Default::default()
            }))?;
        log("startup: GPU device ready");
        let formats = surface.get_capabilities(&adapter).formats;
        let mut config = surface
            .get_default_config(&adapter, 3840, 2160)
            .context("native4K presentation unsupported")?;
        config.format = formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 2;
        surface.configure(&device, &config);
        log("startup: native surface configured");
        let renderer = BeatRenderer::new(
            Gpu {
                device,
                queue,
                adapter,
            },
            package,
            [3840, 2160],
        )?;
        log("startup: native graph prepared");
        ensure!(
            renderer.output_format() == wgpu::TextureFormat::Rgba8UnormSrgb,
            "Android .beat output must be RGBA8 sRGB"
        );
        let (pipeline, group) = present(&renderer, config.format);
        log("startup: presentation prepared");
        Ok(Self {
            surface,
            _window: window,
            renderer,
            pipeline,
            group,
            root: root.into(),
            rank,
            name,
            controller: Controller::new([3840, 2160])?,
            analyzer: Analyzer::new(48000, 2, 1)?,
            last_snapshot: None,
            mode: None,
            frame: 0,
            input_offset: 0,
            input_file: None,
            clock_start: None,
            last_frame: None,
            pixels: vec![],
        })
    }
    fn switch(&mut self, rank: usize) -> Result<()> {
        let package = load(&self.root, rank)?;
        let name = package.manifest.name.clone();
        let gpu = Gpu {
            device: self.renderer.gpu.device.clone(),
            queue: self.renderer.gpu.queue.clone(),
            adapter: self.renderer.gpu.adapter.clone(),
        };
        let renderer = BeatRenderer::new(gpu, package, [3840, 2160])?;
        ensure!(
            renderer.output_format() == wgpu::TextureFormat::Rgba8UnormSrgb,
            "Android .beat output must be RGBA8 sRGB"
        );
        let layout = self.pipeline.get_bind_group_layout(0);
        self.group = renderer
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("beatamp-output"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(renderer.output_view()),
                }],
            });
        self.renderer = renderer;
        self.rank = rank;
        self.name = name;
        self.frame = 0;
        self.last_frame = None;
        self.mode = None;
        self.last_snapshot = None;
        self.clock_start = None;
        self.controller = Controller::new([3840, 2160])?;
        Ok(())
    }
    fn draw(&mut self, mode: i32, now_ns: u64) -> Result<String> {
        ensure!(
            [0, 1, 3].contains(&mode),
            ".beat only accepts livePCM, synthetic control or fullPCM playlist; legacy replay is unavailable"
        );
        if self.mode != Some(mode) {
            self.renderer.reset()?;
            self.controller = Controller::new([3840, 2160])?;
            self.last_snapshot = None;
            self.frame = 0;
            self.clock_start = Some(now_ns);
            self.analyzer = Analyzer::new(if mode == 3 { 44100 } else { 48000 }, 2, 1)?;
            if mode == 3 {
                self.input_file = Some(std::fs::File::open(self.root.join("playlist.s16le"))?);
            }
            self.mode = Some(mode);
        }
        let begin = Instant::now();
        let interval = self
            .last_frame
            .map(|v| begin.duration_since(v).as_secs_f64() * 1000.);
        self.last_frame = Some(begin);
        let control_ns = if mode == 0 {
            now_ns
        } else {
            (self.frame + 1) * 1_000_000_000 / 30
        };
        if mode == 0 {
            if let Some(snapshot) = super::android::latest_report().and_then(|r| r.snapshot) {
                // A concurrently produced snapshot can be newer than frame-start.
                if snapshot.capture_end_ns <= now_ns {
                    self.last_snapshot = Some(snapshot);
                }
            }
        } else {
            let rate = if mode == 3 { 44100u64 } else { 48000u64 };
            let start = self.frame * rate / 30;
            let end = (self.frame + 1) * rate / 30;
            let samples = if mode == 3 {
                let source_start = (self.input_offset + self.frame) * rate / 30;
                let file = self
                    .input_file
                    .as_mut()
                    .context("PCM playlist unavailable")?;
                file.seek(SeekFrom::Start(source_start * 4))?;
                let mut bytes = vec![0; ((end - start) * 4) as usize];
                file.read_exact(&mut bytes)
                    .context("PCM playlist exhausted; no fabricated padding")?;
                bytes
                    .chunks_exact(2)
                    .map(|p| i16::from_le_bytes([p[0], p[1]]))
                    .collect::<Vec<_>>()
            } else {
                (start..end)
                    .flat_map(|n| {
                        let t = n as f64 / rate as f64;
                        let kick = (-(t % 0.5) * 20.).exp();
                        let bass = (t * std::f64::consts::TAU * 110.).sin() * (0.12 + 0.4 * kick);
                        let note = 220. * 2f64.powf([0., 4., 7., 12.][t as usize % 4] / 12.);
                        let melody = (t * std::f64::consts::TAU * note).sin() * 0.25;
                        [(bass * 32767.) as i16, (melody * 32767.) as i16]
                    })
                    .collect()
            };
            if let Some(snapshot) = self.analyzer.push_i16(&samples, start, control_ns, false)? {
                self.last_snapshot = Some(snapshot);
            }
        }
        let mut input = BeatInput::default();
        if let Some(snapshot) = &self.last_snapshot {
            let controls = self.controller.update(snapshot, control_ns)?;
            input.parameters = controls.parameters;
            input.buffers = controls.buffers;
            input.audio = Some(BeatAudioDescriptor {
                generation: snapshot.generation,
                end_frame: snapshot.end_frame,
                capture_end_ns: snapshot.capture_end_ns,
                sample_rate: snapshot.sample_rate,
                channels: snapshot.channels,
                timestamp_estimated: snapshot.timestamp_estimated,
                encoding: Some("signed-16-bit-LE".into()),
            });
        } else {
            input.parameters.insert(
                "scene_time".into(),
                vec![
                    now_ns.saturating_sub(self.clock_start.unwrap_or(now_ns)) as f32 / 1e9,
                    1. / 30.,
                    3840.,
                    2160.,
                ],
            );
        }
        self.renderer.update(&input)?;
        self.renderer.render()?;
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => anyhow::bail!(".beat surface unavailable"),
        };
        ensure!(
            output.texture.width() == 3840 && output.texture.height() == 2160,
            ".beat output lost native4K extent"
        );
        let width = output.texture.width();
        let height = output.texture.height();
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
                label: Some("beatamp-native-screen"),
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
        let allocation = self.renderer.allocation_report();
        let s = self.last_snapshot.as_ref();
        Ok(serde_json::json!({"state":"rendering","engine":"beat","mode":"native-beat-adaptation","preset":self.name,"rank":self.rank,"frame":self.frame,"frame_start_ns":now_ns,"width":width,"height":height,"surface":[width,height],"native_feedback":allocation.dimensions.get("transport").map(|v| v.len()==2 && v.iter().all(|extent| *extent==[3840,2160])),"native_allocations":allocation.dimensions,"logical_allocation_bytes":allocation.bytes,"target_fps":30,"cpu_frame_ms":begin.elapsed().as_secs_f64()*1000.,"frame_interval_ms":interval,"input":match mode{1=>"explicit synthetic stereo PCM control",3=>"Beatport unnormalized44100 stereoPCM",_=>if s.is_some(){"continuous playback PCM"}else{"waiting for playback PCM"}},"feature_age_ms":s.map(|s|control_ns.saturating_sub(s.capture_end_ns)as f64/1e6),"audio_generation":s.map(|s|s.generation),"fast_bins":s.and_then(|s|s.fast_spectrum.as_ref()).map(|s|s.left_power.len()),"detail_bins":s.and_then(|s|s.detail_spectrum.as_ref()).map(|s|s.left_power.len()),"sample_rate":s.map(|s|s.sample_rate),"nyquist_hz":s.map(|s|s.sample_rate as f64/2.),"graphics_backend":format!("{:?}",self.renderer.gpu.adapter.get_info().backend),"recording":mode==3,"playback_benchmark_eligible":mode!=3,"timing_scope":"CPU work/present call; no presented-frame deadline claim"}).to_string())
    }
}
fn error(message: String) {
    log(&message);
    ERROR.with(|e| *e.borrow_mut() = message);
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeBeat_create(
    mut env: JNIEnv,
    _class: JClass,
    surface: JObject,
    directory: JString,
    rank: jint,
    backend: jint,
) -> jlong {
    static PANIC_LOG: std::sync::Once = std::sync::Once::new();
    PANIC_LOG.call_once(|| std::panic::set_hook(Box::new(|info| log(&format!("{info}")))));
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
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeBeat_frame(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    rank: jint,
    mode: jint,
    adapted: jint,
    now_ns: jlong,
) -> jstring {
    let result = PLAYERS.with(|p| -> Result<String> {
        ensure!(
            adapted != 0,
            "source translation requires the separately selected diagnostic engine"
        );
        let mut p = p.borrow_mut();
        let player = p.get_mut(&handle).context(".beat renderer unavailable")?;
        if player.rank != rank as usize {
            player.switch(rank as usize)?;
        }
        ensure!(now_ns >= 0, "invalid clock");
        player.draw(mode, now_ns as u64)
    });
    let json = match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({"state":"failed","engine":"beat","error":format!("{e:#}")})
            .to_string(),
    };
    env.new_string(json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeBeat_error(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    env.new_string(ERROR.with(|e| e.borrow().clone()))
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeBeat_destroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    PLAYERS.with(|p| p.borrow_mut().remove(&handle));
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeBeat_inputOffset(
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
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeBeat_pixels(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jni::sys::jobject {
    let result = PLAYERS.with(|p| -> Result<jni::objects::JByteBuffer> {
        let mut p = p.borrow_mut();
        let player = p.get_mut(&handle).context(".beat capture unavailable")?;
        player.pixels = player.renderer.capture()?;
        // SAFETY: Java copies synchronously on this render thread, before next capture/drop.
        Ok(unsafe { env.new_direct_byte_buffer(player.pixels.as_mut_ptr(), player.pixels.len()) }?)
    });
    match result {
        Ok(v) => v.into_raw(),
        Err(e) => {
            error(format!("{e:#}"));
            std::ptr::null_mut()
        }
    }
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeBeat_capture(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    path: JString,
) -> jstring {
    let result = (|| -> Result<String> {
        let path: String = env.get_string(&path)?.into();
        PLAYERS.with(|p|->Result<String>{let p=p.borrow();let player=p.get(&handle).context(".beat capture unavailable")?;let rgba=player.renderer.capture()?;let image=image::RgbaImage::from_raw(3840,2160,rgba).context(".beat capture extent mismatch")?;let file=std::fs::File::create(&path)?;image::codecs::jpeg::JpegEncoder::new_with_quality(file,92).encode_image(&image::DynamicImage::ImageRgba8(image))?;Ok(serde_json::json!({"rank":player.rank,"engine":"beat","dimensions":[3840,2160],"capture_backend":"wgpu .beat RGBA8-sRGB readback; JPEG preview","performance_sample":false,"path":path}).to_string())})
    })();
    let json = match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({"error":format!("{e:#}")}).to_string(),
    };
    env.new_string(json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
