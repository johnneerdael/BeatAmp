@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var smooth_sampler: sampler;
@fragment fn fragment_main(@builtin(position) pixel:vec4<f32>)->@location(0) vec4<f32> {
    let uv=pixel.xy/control.time.zw;
    let hdr=textureSample(scene,smooth_sampler,uv).rgb*control.palette_a.w;
    let tone=hdr/(vec3<f32>(1.0)+hdr);
    let vignette=1.0-0.2*pow(length((uv-vec2<f32>(0.5))*vec2<f32>(1.2,1.0)),2.0);
    // Attachment is sRGB; hardware performs the sole output transfer encoding.
    return vec4<f32>(tone*vignette,1.0);
}
