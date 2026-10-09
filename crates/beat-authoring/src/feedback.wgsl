@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var history: texture_2d<f32>;
@group(0) @binding(3) var smooth_sampler: sampler;
@fragment fn fragment_main(@builtin(position) pixel:vec4<f32>)->@location(0) vec4<f32> {
    let uv=pixel.xy/control.time.zw;
    let offset=(uv-vec2<f32>(0.5))*(1.0+control.time.y*(0.035+control.energy.z*0.035));
    let angle=control.time.y*(0.018+control.space.x*0.025);
    let source_uv=rotation(offset,angle)+vec2<f32>(0.5);
    let previous=textureSample(history,smooth_sampler,clamp(source_uv,vec2<f32>(0.0),vec2<f32>(1.0))).rgb;
    let current=textureSample(scene,smooth_sampler,uv).rgb;
    let decay=pow(control.style.w,max(control.time.y,0.0)*30.0);
    let trail=previous*decay;
    // Coherent continuous-time transport. Bounded HDR recurrence avoids saturation.
    return vec4<f32>(max(current,trail)+min(current,trail)*0.035,1.0);
}
