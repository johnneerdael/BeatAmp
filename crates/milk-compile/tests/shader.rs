use milk_compile::shader::from_wgsl;
use std::collections::BTreeSet;

#[test]
fn compiler_resource_names_survive_unique_binding_legalization() {
    let s=from_wgsl("struct P { color:vec4<f32> }; @group(0) @binding(0) var<uniform> params:P; @group(0) @binding(0) var sampler_main:texture_2d<f32>; @group(0) @binding(0) var sampler_main_sampler:sampler; @fragment fn main(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{return textureSample(sampler_main,sampler_main_sampler,uv)*params.color;}").unwrap();
    assert_eq!(s.bindings.len(), 3);
    assert_eq!(
        s.bindings
            .iter()
            .map(|b| (b.group, b.binding))
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    assert!(
        s.bindings
            .iter()
            .any(|b| b.name == "sampler_main" && b.kind == "texture2d")
    );
    assert!(
        s.bindings
            .iter()
            .any(|b| b.name == "sampler_main_sampler" && b.kind == "sampler")
    );
}
#[test]
fn an_invalid_shader_cannot_become_a_valid_fallback_artifact() {
    assert!(
        from_wgsl("@fragment fn main()->@location(0) vec4<f32>{return missing_function();}")
            .is_err()
    );
    assert!(from_wgsl("struct Out{@location(0) color:array<vec4<f32>,1>}; @fragment fn main()->Out {return Out(array<vec4<f32>,1>(vec4<f32>(1.0)));}").is_err());
}

#[test]
fn dynamic_shader_loops_receive_a_visible_iteration_budget() {
    let s=milk_compile::shader::bounded_wgsl("@fragment fn main()->@location(0) vec4<f32>{var x=0u;loop{x+=1u;if x>10000u{break;}}return vec4<f32>(f32(x));}",64).unwrap();
    assert_eq!(s.loop_budget, Some(64));
    assert!(s.wgsl.contains("poc_iterations"));
}
