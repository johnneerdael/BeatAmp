struct Params { data: array<vec4<f32>,64> };
@group(0) @binding(0) var<uniform> params:Params;
@group(0) @binding(1) var history:texture_2d<f32>;
@group(0) @binding(2) var linear_sampler:sampler;
struct MeshOut {
 @builtin(position) position:vec4<f32>,
 @location(0) original:vec2<f32>,
 @location(1) warped:vec2<f32>,
};
@vertex fn mesh_vertex(@location(0) position:vec2<f32>,@location(1) original:vec2<f32>,@location(2) warped:vec2<f32>) -> MeshOut {
 var output:MeshOut; output.position=vec4<f32>(position.x*2.0-1.0,1.0-position.y*2.0,0.0,1.0);
 output.original=original;output.warped=warped;return output;
}
@fragment fn default_warp(in:MeshOut)->@location(0) vec4<f32> {
 let color=textureSample(history,linear_sampler,in.warped).rgb;
 return vec4<f32>(color*params.data[7].w,1.0);
}
@fragment fn default_comp(in:MeshOut)->@location(0) vec4<f32> {
 var uv=(in.original-vec2<f32>(0.5))/max(params.data[18].y,0.001);
 let orient=u32(params.data[18].w);
 if ((orient&1u)!=0u) {uv.x=-uv.x;}
 if ((orient&2u)!=0u) {uv.y=-uv.y;}
 let main=textureSample(history,linear_sampler,in.original).rgb;
 let echo=textureSample(history,linear_sampler,uv+vec2<f32>(0.5)).rgb;
 var color=mix(main,echo,clamp(params.data[18].z,0.0,1.0));
 color=pow(max(color,vec3<f32>(0.0)),vec3<f32>(1.0/max(params.data[18].x,0.001)));
 return vec4<f32>(color,1.0);
}
struct GeoOut { @builtin(position) position:vec4<f32>,@location(0) color:vec4<f32>, @location(1) texture:vec3<f32> };
@vertex fn geometry_vertex(@location(0) position:vec2<f32>,@location(1) color:vec4<f32>,@location(2) texture:vec3<f32>)->GeoOut {
 var out:GeoOut;out.position=vec4<f32>(position.x*2.0-1.0,1.0-position.y*2.0,0.0,1.0);out.color=color;out.texture=texture;return out;
}
@fragment fn geometry_fragment(in:GeoOut)->@location(0) vec4<f32> {if (in.texture.z>0.5) {let feedback=textureSampleLevel(history,linear_sampler,in.texture.xy,0.0);return vec4<f32>(in.color.rgb*feedback.rgb,in.color.a);}return in.color;}
