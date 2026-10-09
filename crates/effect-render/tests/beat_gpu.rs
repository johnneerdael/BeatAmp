use effect_model::beat::Manifest;
use effect_package::Package;
use effect_render::{
    beat::{BeatAudioDescriptor, BeatInput, BeatRenderer},
    gpu::Gpu,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Cursor};

const VERTEX: &str = r#"
struct Out { @builtin(position) position:vec4<f32> }
@vertex fn vertex_main(@builtin(vertex_index) i:u32)->Out {
let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
var o:Out; o.position=vec4(p[i],0.,1.); return o;
}
"#;
fn binding(slot: u32, resource: &str, previous: bool) -> Value {
    json!({"binding":slot,"source":{"kind":"resource","resource":resource,"previous":previous}})
}
fn texture(role: &str, history: bool) -> Value {
    json!({"kind":"texture","format":"rgba8unorm","extent":{"mode":"native"},"history":history,"role":role,"mips":1,"color_space":"linear","alpha":"opaque"})
}
fn package(
    mut manifest: Value,
    programs: Vec<(&str, String)>,
    extra: Vec<(&str, Vec<u8>)>,
) -> Package {
    let mut assets = BTreeMap::new();
    for (id, code) in programs {
        let bytes = code.into_bytes();
        manifest["assets"][id] = json!({"path":format!("programs/{id}.wgsl"),"sha256":format!("{:x}",Sha256::digest(&bytes)),"kind":"wgsl"});
        assets.insert(id.to_owned(), bytes);
    }
    for (id, bytes) in extra {
        assets.insert(id.to_owned(), bytes);
    }
    Package {
        manifest: serde_json::from_value::<Manifest>(manifest).unwrap(),
        assets,
    }
}
fn base() -> Value {
    json!({"format":"beat","version":1,"name":"Independent native graph","author":"Fixture artist","quality":"native","output":"final","assets":{},"parameters":{},"resources":{"final":texture("output",false)},"passes":[]})
}

#[test]
fn standalone_output_can_be_bound_by_a_native_presentation_consumer() {
    let mut m = base();
    m["passes"] =
        json!([{"id":"display","kind":"render","program":"display","reads":[],"writes":["final"]}]);
    let program = format!(
        "{VERTEX}\n@fragment fn fragment_main()->@location(0) vec4<f32>{{return vec4(0.2,0.4,0.8,1.);}}"
    );
    let renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("display", program)], vec![]),
        [3840, 2160],
    )
    .unwrap();
    let device = &renderer.gpu.device;
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    });
    let _consumer = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("native presentation consumer"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(renderer.output_view()),
        }],
    });
    let error = pollster::block_on(scope.pop());
    assert!(
        error.is_none(),
        "output cannot be presented through its public view: {error:?}"
    );
}

#[test]
fn native_four_k_feedback_uses_previous_frame_and_resets_without_source_files() {
    // A hidden low-resolution canvas, wrong ping-pong parity or reset leak breaks this.
    let mut m = base();
    m["parameters"] = json!({"drive":{"initial":[1.0]}});
    m["resources"]["history"] = texture("primary_feedback", true);
    m["passes"] = json!([
        {"id":"display","kind":"render","program":"display","reads":[{"resource":"history","previous":false,"kind":"color_texture"}],"writes":["final"],"bindings":[binding(0,"history",false)]},
        {"id":"inject","kind":"render","program":"inject","reads":[{"resource":"history","previous":true,"kind":"color_texture"}],"writes":["history"],"parameters":["drive"],"bindings":[binding(0,"history",true),{"binding":1,"source":{"kind":"parameters"}}]}
    ]);
    let inject = format!(
        "{VERTEX}\n@group(0) @binding(0) var history:texture_2d<f32>;\n@group(0) @binding(1) var<uniform> params:vec4<f32>;\n@fragment fn fragment_main(o:Out)->@location(0) vec4<f32>{{let prior=textureLoad(history,vec2<i32>(o.position.xy),0).r;return vec4(prior+params.x*0.25,0.,0.,1.);}}"
    );
    let display = format!(
        "{VERTEX}\n@group(0) @binding(0) var history:texture_2d<f32>;\n@fragment fn fragment_main(o:Out)->@location(0) vec4<f32>{{return textureLoad(history,vec2<i32>(o.position.xy),0);}}"
    );
    let p = package(m, vec![("inject", inject), ("display", display)], vec![]);
    let mut renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        p,
        [3840, 2160],
    )
    .unwrap();
    let report = renderer.allocation_report();
    assert_eq!(report.dimensions["history"], [[3840, 2160]; 2]);
    assert_eq!(report.dimensions["final"], [[3840, 2160]]);
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 64).abs() <= 1);
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 128).abs() <= 2);
    renderer
        .update(&BeatInput {
            parameters: BTreeMap::from([("drive".into(), vec![0.])]),
            ..Default::default()
        })
        .unwrap();
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 128).abs() <= 2);
    renderer.reset().unwrap();
    renderer
        .update(&BeatInput {
            parameters: BTreeMap::from([("drive".into(), vec![0.])]),
            ..Default::default()
        })
        .unwrap();
    renderer.render().unwrap();
    assert_eq!(renderer.capture().unwrap()[0], 0);
}

#[test]
fn compute_buffer_output_reaches_a_later_render_pass_in_dependency_order() {
    // Running JSON pass order instead of the graph plan renders stale zero data.
    let mut m = base();
    m["resources"]["state"] =
        json!({"kind":"buffer","bytes":16,"history":false,"initialized":true});
    m["passes"] = json!([
        {"id":"draw","kind":"render","program":"draw","reads":[{"resource":"state","previous":false,"kind":"buffer"}],"writes":["final"],"bindings":[binding(0,"state",false)]},
        {"id":"step","kind":"compute","program":"step","reads":[],"writes":["state"],"dispatch":[1,1,1],"bindings":[binding(0,"state",false)]}
    ]);
    let step="@group(0) @binding(0) var<storage,read_write> state:array<vec4<f32>>; @compute @workgroup_size(1) fn compute_main(){state[0]=vec4(0.,0.75,0.,1.);}".to_owned();
    let draw = format!(
        "{VERTEX}\n@group(0) @binding(0) var<storage,read> state:array<vec4<f32>>; @fragment fn fragment_main()->@location(0) vec4<f32>{{return state[0];}}"
    );
    let mut renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("draw", draw), ("step", step)], vec![]),
        [128, 128],
    )
    .unwrap();
    renderer.render().unwrap();
    let pixels = renderer.capture().unwrap();
    assert!((pixels[1] as i32 - 191).abs() <= 1);
    assert_eq!(pixels[0], 0);
}

#[test]
fn grayscale16_is_normalized_numerically_before_half_float_upload() {
    // Uploading u16 PNG bit patterns as half floats produces the wrong sampled height.
    let im =
        image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_pixel(4, 2, image::Luma([16384]));
    let mut encoded = Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma16(im)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let bytes = encoded.into_inner();
    let mut m = base();
    m["assets"]["height"] = json!({"path":"textures/height.png","kind":"image","sha256":format!("{:x}",Sha256::digest(&bytes))});
    m["resources"]["height"] = json!({"kind":"texture","format":"r16float","extent":{"mode":"fixed","width":4,"height":2},"history":false,"role":"asset","mips":1,"color_space":"data","alpha":"opaque","asset":"height"});
    m["passes"] = json!([{"id":"draw","kind":"render","program":"draw","reads":[{"resource":"height","previous":false,"kind":"color_texture"}],"writes":["final"],"bindings":[binding(0,"height",false)]}]);
    let draw = format!(
        "{VERTEX}\n@group(0) @binding(0) var height:texture_2d<f32>; @fragment fn fragment_main()->@location(0) vec4<f32>{{return vec4(textureLoad(height,vec2(0,0),0).r,0.,0.,1.);}}"
    );
    let mut renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("draw", draw)], vec![("height", bytes)]),
        [128, 128],
    )
    .unwrap();
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 64).abs() <= 1);
}

#[test]
fn cooked_numeric_mips_preserve_signed_half_values_and_validate_byte_count() {
    let mut raw = Vec::new();
    // Two RG16F mip levels: 2x2 and 1x1. Read the signed second level.
    for _ in 0..5 {
        for value in [-0.5f32, 0.75] {
            raw.extend_from_slice(&half::f16::from_f32(value).to_bits().to_le_bytes());
        }
    }
    let mut m = base();
    m["assets"]["flow"] = json!({"path":"textures/flow.bin","kind":"binary","sha256":format!("{:x}",Sha256::digest(&raw))});
    m["resources"]["flow"] = json!({"kind":"texture","format":"rg16float","extent":{"mode":"fixed","width":2,"height":2},"history":false,"role":"asset","mips":2,"color_space":"data","alpha":"opaque","asset":"flow"});
    m["passes"] = json!([{"id":"draw","kind":"render","program":"draw","reads":[{"resource":"flow","previous":false,"kind":"color_texture"}],"writes":["final"],"bindings":[binding(0,"flow",false)]}]);
    let draw = format!(
        "{VERTEX}\n@group(0) @binding(0) var flow:texture_2d<f32>; @fragment fn fragment_main()->@location(0) vec4<f32>{{let v=textureLoad(flow,vec2(0,0),1).xy;return vec4(v.x+1.,v.y,0.,1.);}}"
    );
    let mut renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(
            m.clone(),
            vec![("draw", draw.clone())],
            vec![("flow", raw.clone())],
        ),
        [32, 32],
    )
    .unwrap();
    renderer.render().unwrap();
    let pixels = renderer.capture().unwrap();
    assert!((pixels[0] as i32 - 128).abs() <= 1);
    assert!((pixels[1] as i32 - 191).abs() <= 1);
    raw.pop();
    m["assets"]["flow"]["sha256"] = json!(format!("{:x}", Sha256::digest(&raw)));
    let result = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("draw", draw)], vec![("flow", raw)]),
        [32, 32],
    );
    assert!(result.err().unwrap().to_string().contains("byte size"));
}

#[test]
fn arbitrary_parameters_use_individual_vec4_lanes_and_reject_partial_invalid_updates() {
    let mut m = base();
    m["parameters"] = json!({"phase":{"initial":[0.1,0.2]},"color":{"initial":[0.3,0.4,0.5]},"spectrum":{"initial":[0.6,0.7,0.8,0.9,0.25]}});
    m["passes"] = json!([{"id":"draw","kind":"render","program":"draw","reads":[],"writes":["final"],"parameters":["phase","color","spectrum"],"bindings":[{"group":1,"binding":4,"source":{"kind":"parameters"}}]}]);
    let draw = format!(
        "{VERTEX}\nstruct Controls{{phase:vec4<f32>,color:vec4<f32>,spectrum:array<vec4<f32>,2>}} @group(1) @binding(4) var<uniform> c:Controls; @fragment fn fragment_main()->@location(0) vec4<f32>{{return vec4(c.phase.y,c.color.z,c.spectrum[1].x,1.);}}"
    );
    let mut renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("draw", draw)], vec![]),
        [32, 32],
    )
    .unwrap();
    renderer.render().unwrap();
    let pixels = renderer.capture().unwrap();
    assert!((pixels[0] as i32 - 51).abs() <= 1);
    assert!((pixels[1] as i32 - 128).abs() <= 1);
    assert!((pixels[2] as i32 - 64).abs() <= 1);
    let bad = BeatInput {
        parameters: BTreeMap::from([
            ("phase".into(), vec![0.9, 0.9]),
            ("spectrum".into(), vec![f32::NAN; 5]),
        ]),
        ..Default::default()
    };
    assert!(renderer.update(&bad).is_err());
    renderer.render().unwrap();
    assert_eq!(renderer.capture().unwrap(), pixels);
    renderer
        .update(&BeatInput {
            parameters: BTreeMap::from([("phase".into(), vec![0.8, 0.75])]),
            ..Default::default()
        })
        .unwrap();
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 191).abs() <= 1);
    renderer.reset().unwrap();
    renderer.render().unwrap();
    assert_eq!(renderer.capture().unwrap(), pixels);
}

#[test]
fn unsupported_shader_binding_type_is_a_load_diagnostic() {
    let mut m = base();
    m["parameters"] = json!({"gain":{"initial":[1.]}});
    m["passes"] = json!([{"id":"draw","kind":"render","program":"draw","reads":[],"writes":["final"],"parameters":["gain"],"bindings":[{"binding":0,"source":{"kind":"parameters"}}]}]);
    let draw = format!(
        "{VERTEX}\n@group(0) @binding(0) var<storage,read> c:array<vec4<f32>>; @fragment fn fragment_main()->@location(0) vec4<f32>{{return c[0];}}"
    );
    let result = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("draw", draw)], vec![]),
        [32, 32],
    );
    assert!(result.err().unwrap().to_string().contains("uniform"));
}

#[test]
fn compute_history_is_distinct_from_current_buffer_and_reset_clears_both_copies() {
    let mut m = base();
    m["resources"]["state"] = json!({"kind":"buffer","bytes":16,"history":true,"initialized":true});
    m["passes"] = json!([
        {"id":"draw","kind":"render","program":"draw","reads":[{"resource":"state","previous":false,"kind":"buffer"}],"writes":["final"],"bindings":[binding(0,"state",false)]},
        {"id":"step","kind":"compute","program":"step","reads":[{"resource":"state","previous":true,"kind":"buffer"}],"writes":["state"],"dispatch":[1,1,1],"bindings":[binding(0,"state",true),binding(1,"state",false)]}
    ]);
    let step="@group(0) @binding(0) var<storage,read> prior:array<vec4<f32>>; @group(0) @binding(1) var<storage,read_write> state:array<vec4<f32>>; @compute @workgroup_size(1) fn compute_main(){state[0]=prior[0]+vec4(0.25,0.,0.,1.);}".to_owned();
    let draw = format!(
        "{VERTEX}\n@group(0) @binding(0) var<storage,read> state:array<vec4<f32>>; @fragment fn fragment_main()->@location(0) vec4<f32>{{return state[0];}}"
    );
    let mut renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("draw", draw), ("step", step)], vec![]),
        [32, 32],
    )
    .unwrap();
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 64).abs() <= 1);
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 128).abs() <= 1);
    renderer.reset().unwrap();
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 64).abs() <= 1);
}

#[test]
fn sampler_axes_are_independent_and_data_alpha_is_not_overwritten() {
    let im = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(
        2,
        2,
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 32, 96, 160, 64, 0, 0, 255, 255,
        ],
    )
    .unwrap();
    let mut encoded = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(im)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let bytes = encoded.into_inner();
    let mut m = base();
    m["resources"]["final"]["alpha"] = json!("straight");
    m["assets"]["pixels"] = json!({"path":"textures/pixels.png","kind":"image","sha256":format!("{:x}",Sha256::digest(&bytes))});
    m["resources"]["pixels"] = json!({"kind":"texture","format":"rgba8unorm","extent":{"mode":"fixed","width":2,"height":2},"history":false,"role":"asset","mips":1,"color_space":"data","alpha":"straight","asset":"pixels"});
    m["passes"] = json!([{"id":"draw","kind":"render","program":"draw","reads":[{"resource":"pixels","previous":false,"kind":"color_texture"}],"writes":["final"],"bindings":[binding(0,"pixels",false),{"binding":1,"source":{"kind":"sampler","u":"repeat","v":"clamp-to-edge","filter":"nearest"}}]}]);
    let draw = format!(
        "{VERTEX}\n@group(0) @binding(0) var pixels:texture_2d<f32>;@group(0) @binding(1) var addressing:sampler; @fragment fn fragment_main()->@location(0) vec4<f32>{{return textureSampleLevel(pixels,addressing,vec2(1.25,1.25),0.);}}"
    );
    let mut renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("draw", draw)], vec![("pixels", bytes)]),
        [32, 32],
    )
    .unwrap();
    renderer.render().unwrap();
    assert_eq!(&renderer.capture().unwrap()[..4], &[32, 96, 160, 64]);
}

#[test]
fn vertex_pulled_instances_use_declared_depth_attachment() {
    let mut m = base();
    m["resources"]["depth"] = json!({"kind":"texture","format":"depth32float","extent":{"mode":"native"},"history":false,"role":"intermediate","mips":1,"color_space":"data","alpha":"opaque"});
    m["passes"] = json!([{"id":"draw","kind":"render","program":"draw","reads":[],"writes":["final","depth"],"draw":{"vertices":3,"instances":2,"topology":"triangle-list"}}]);
    let draw=r#"struct Out{@builtin(position) position:vec4<f32>,@location(0) @interpolate(flat) color:vec4<f32>}
@vertex fn vertex_main(@builtin(vertex_index) i:u32,@builtin(instance_index) instance:u32)->Out{
let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));var o:Out;
o.position=vec4(p[i],select(0.2,0.8,instance==1),1.);o.color=select(vec4(1.,0.,0.,1.),vec4(0.,1.,0.,1.),instance==1);return o;}
@fragment fn fragment_main(o:Out)->@location(0) vec4<f32>{return o.color;}"#.to_owned();
    let mut renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("draw", draw)], vec![]),
        [32, 32],
    )
    .unwrap();
    renderer.render().unwrap();
    assert_eq!(&renderer.capture().unwrap()[..4], &[255, 0, 0, 255]);
    renderer.reset().unwrap();
    renderer.render().unwrap();
    assert_eq!(&renderer.capture().unwrap()[..4], &[255, 0, 0, 255]);
}

#[test]
fn storage_texture_compute_writes_are_visible_to_render_sampling() {
    let mut m = base();
    m["resources"]["field"] = texture("intermediate", false);
    m["resources"]["field"]["extent"] = json!({"mode":"fixed","width":8,"height":8});
    m["passes"] = json!([
        {"id":"draw","kind":"render","program":"draw","reads":[{"resource":"field","previous":false,"kind":"color_texture"}],"writes":["final"],"bindings":[binding(0,"field",false)]},
        {"id":"step","kind":"compute","program":"step","reads":[],"writes":["field"],"dispatch":[8,8,1],"bindings":[binding(0,"field",false)]}
    ]);
    let step="@group(0) @binding(0) var field:texture_storage_2d<rgba8unorm,write>; @compute @workgroup_size(1) fn compute_main(@builtin(global_invocation_id) i:vec3<u32>){textureStore(field,vec2<i32>(i.xy),vec4(0.5,0.25,0.,1.));}".to_owned();
    let draw = format!(
        "{VERTEX}\n@group(0) @binding(0) var field:texture_2d<f32>; @fragment fn fragment_main()->@location(0) vec4<f32>{{return textureLoad(field,vec2(0,0),0);}}"
    );
    let mut renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("draw", draw), ("step", step)], vec![]),
        [32, 32],
    )
    .unwrap();
    renderer.render().unwrap();
    let pixels = renderer.capture().unwrap();
    assert!((pixels[0] as i32 - 128).abs() <= 1);
    assert!((pixels[1] as i32 - 64).abs() <= 1);
}

#[test]
fn shader_cannot_hide_current_frame_buffer_reads_behind_a_write_binding() {
    let mut m = base();
    m["resources"]["state"] =
        json!({"kind":"buffer","bytes":16,"history":false,"initialized":true});
    m["passes"] = json!([
        {"id":"draw","kind":"render","program":"draw","reads":[{"resource":"state","previous":false,"kind":"buffer"}],"writes":["final"],"bindings":[binding(0,"state",false)]},
        {"id":"step","kind":"compute","program":"step","reads":[],"writes":["state"],"dispatch":[1,1,1],"bindings":[binding(0,"state",false)]}
    ]);
    let step="@group(0) @binding(0) var<storage,read_write> state:array<vec4<f32>>; @compute @workgroup_size(1) fn compute_main(){state[0]=state[0]+vec4(0.,0.25,0.,1.);}".to_owned();
    let draw = format!(
        "{VERTEX}\n@group(0) @binding(0) var<storage,read> state:array<vec4<f32>>; @fragment fn fragment_main()->@location(0) vec4<f32>{{return state[0];}}"
    );
    let result = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("draw", draw), ("step", step)], vec![]),
        [32, 32],
    );
    assert!(
        result
            .err()
            .is_some_and(|e| e.to_string().contains("undeclared shader read"))
    );
}

#[test]
fn audio_generation_resets_history_and_stale_updates_are_rejected_atomically() {
    let mut m = base();
    m["resources"]["history"] = texture("primary_feedback", true);
    m["passes"] = json!([
        {"id":"display","kind":"render","program":"display","reads":[{"resource":"history","previous":false,"kind":"color_texture"}],"writes":["final"],"bindings":[binding(0,"history",false)]},
        {"id":"inject","kind":"render","program":"inject","reads":[{"resource":"history","previous":true,"kind":"color_texture"}],"writes":["history"],"bindings":[binding(0,"history",true)]}
    ]);
    let inject = format!(
        "{VERTEX}\n@group(0) @binding(0) var history:texture_2d<f32>;@fragment fn fragment_main(o:Out)->@location(0) vec4<f32>{{return vec4(textureLoad(history,vec2<i32>(o.position.xy),0).r+0.25,0.,0.,1.);}}"
    );
    let display = format!(
        "{VERTEX}\n@group(0) @binding(0) var history:texture_2d<f32>;@fragment fn fragment_main(o:Out)->@location(0) vec4<f32>{{return textureLoad(history,vec2<i32>(o.position.xy),0);}}"
    );
    let mut renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("inject", inject), ("display", display)], vec![]),
        [32, 32],
    )
    .unwrap();
    let descriptor = BeatAudioDescriptor {
        generation: 10,
        end_frame: 4096,
        capture_end_ns: 90_000_000,
        sample_rate: 48000,
        channels: 2,
        timestamp_estimated: false,
        encoding: Some("signed_i16".into()),
    };
    renderer
        .update(&BeatInput {
            audio: Some(descriptor.clone()),
            ..Default::default()
        })
        .unwrap();
    renderer.render().unwrap();
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 128).abs() <= 1);
    let next = BeatAudioDescriptor {
        generation: 11,
        end_frame: 1024,
        capture_end_ns: 21_333_333,
        ..descriptor.clone()
    };
    renderer
        .update(&BeatInput {
            audio: Some(next.clone()),
            ..Default::default()
        })
        .unwrap();
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 64).abs() <= 1);
    assert_eq!(renderer.input_descriptor(), Some(&next));
    assert!(
        renderer
            .update(&BeatInput {
                audio: Some(descriptor),
                ..Default::default()
            })
            .is_err()
    );
    let stale = BeatAudioDescriptor {
        end_frame: 1000,
        ..next.clone()
    };
    assert!(
        renderer
            .update(&BeatInput {
                audio: Some(stale),
                ..Default::default()
            })
            .is_err()
    );
    assert_eq!(renderer.input_descriptor(), Some(&next));
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 128).abs() <= 1);
}

#[test]
fn read_only_storage_texture_usage_is_derived_from_the_shader_binding() {
    let raw = 0.625f32.to_le_bytes().to_vec();
    let mut m = base();
    m["assets"]["value"] = json!({"path":"textures/value.bin","kind":"binary","sha256":format!("{:x}",Sha256::digest(&raw))});
    m["resources"]["value"] = json!({"kind":"texture","format":"r32float","extent":{"mode":"fixed","width":1,"height":1},"history":false,"role":"asset","mips":1,"color_space":"data","alpha":"opaque","asset":"value"});
    m["resources"]["state"] =
        json!({"kind":"buffer","bytes":16,"history":false,"initialized":true});
    m["passes"] = json!([
        {"id":"draw","kind":"render","program":"draw","reads":[{"resource":"state","previous":false,"kind":"buffer"}],"writes":["final"],"bindings":[binding(0,"state",false)]},
        {"id":"step","kind":"compute","program":"step","reads":[{"resource":"value","previous":false,"kind":"color_texture"}],"writes":["state"],"dispatch":[1,1,1],"bindings":[binding(0,"value",false),binding(1,"state",false)]}
    ]);
    let step="@group(0) @binding(0) var value:texture_storage_2d<r32float,read>;@group(0) @binding(1) var<storage,read_write> state:array<vec4<f32>>;@compute @workgroup_size(1) fn compute_main(){state[0]=vec4(textureLoad(value,vec2(0,0)).r,0.,0.,1.);}".to_owned();
    let draw = format!(
        "{VERTEX}\n@group(0) @binding(0) var<storage,read> state:array<vec4<f32>>;@fragment fn fragment_main()->@location(0) vec4<f32>{{return state[0];}}"
    );
    let mut renderer = BeatRenderer::new(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(
            m,
            vec![("draw", draw), ("step", step)],
            vec![("value", raw)],
        ),
        [32, 32],
    )
    .unwrap();
    renderer.render().unwrap();
    assert!((renderer.capture().unwrap()[0] as i32 - 159).abs() <= 1);
}

#[test]
fn caller_draw_budget_bounds_the_entire_frame_before_gpu_allocation() {
    let mut m = base();
    m["resources"]["field"] = texture("intermediate", false);
    m["passes"] = json!([
        {"id":"field","kind":"render","program":"field","reads":[],"writes":["field"]},
        {"id":"display","kind":"render","program":"display","reads":[{"resource":"field","previous":false,"kind":"color_texture"}],"writes":["final"],"bindings":[binding(0,"field",false)]}
    ]);
    let field = format!(
        "{VERTEX}\n@fragment fn fragment_main()->@location(0) vec4<f32>{{return vec4(0.5,0.,0.,1.);}}"
    );
    let display = format!(
        "{VERTEX}\n@group(0) @binding(0) var field:texture_2d<f32>;@fragment fn fragment_main()->@location(0) vec4<f32>{{return textureLoad(field,vec2(0,0),0);}}"
    );
    let mut caps = effect_model::beat::Capabilities::baseline([32, 32]);
    caps.max_draw_work = 5;
    let result = BeatRenderer::with_capabilities(
        pollster::block_on(Gpu::headless()).unwrap(),
        package(m, vec![("field", field), ("display", display)], vec![]),
        caps,
    );
    assert!(
        result
            .err()
            .is_some_and(|e| e.to_string().contains("frame draw work budget"))
    );
}
