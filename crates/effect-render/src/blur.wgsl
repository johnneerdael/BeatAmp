@group(0) @binding(0) var input_texture:texture_2d<f32>;
@group(0) @binding(1) var filtering:sampler;
struct V { @builtin(position) position:vec4<f32>,@location(0) uv:vec2<f32> };
@vertex fn vertex(@builtin(vertex_index) i:u32)->V {var p=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0));var out:V;out.position=vec4<f32>(p[i],0.0,1.0);out.uv=vec2<f32>((p[i].x+1.0)*0.5,(1.0-p[i].y)*0.5);return out;}
@fragment fn fragment(in:V)->@location(0) vec4<f32> {let d=1.0/vec2<f32>(textureDimensions(input_texture));let c=textureSample(input_texture,filtering,in.uv)*0.4;let x=(textureSample(input_texture,filtering,in.uv+vec2<f32>(d.x*2.0,0.0))+textureSample(input_texture,filtering,in.uv-vec2<f32>(d.x*2.0,0.0)))*0.15;let y=(textureSample(input_texture,filtering,in.uv+vec2<f32>(0.0,d.y*2.0))+textureSample(input_texture,filtering,in.uv-vec2<f32>(0.0,d.y*2.0)))*0.15;return c+x+y;}
