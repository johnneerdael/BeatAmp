// Independently authored Beatamp shared ABI, no legacy renderer code.
struct Controls {
    time: vec4<f32>, energy: vec4<f32>, regions_lo: vec4<f32>, regions_hi: vec4<f32>,
    chroma_a: vec4<f32>, chroma_b: vec4<f32>, chroma_c: vec4<f32>,
    space: vec4<f32>, windows: vec4<f32>, frequency: vec4<f32>,
    style: vec4<f32>, palette_a: vec4<f32>, palette_b: vec4<f32>, palette_c: vec4<f32>,
}
@group(0) @binding(0) var<uniform> control: Controls;
@vertex fn vertex_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0));
    return vec4<f32>(positions[index],0.0,1.0);
}
fn rotation(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let s=sin(angle); let c=cos(angle);
    return vec2<f32>(c*p.x-s*p.y,s*p.x+c*p.y);
}
fn luminous(distance: f32, width: f32) -> f32 {
    let aa=1.5/control.time.w;
    let core=1.0-smoothstep(width,width+aa,abs(distance));
    let halo=exp(-abs(distance)/max(width*5.0,0.001));
    return core+halo*0.35;
}
fn palette(phase: f32) -> vec3<f32> {
    let a=mix(control.palette_a.xyz,control.palette_b.xyz,0.5+0.5*sin(phase));
    return mix(a,control.palette_c.xyz,0.22+0.18*sin(phase*0.63+1.0));
}
