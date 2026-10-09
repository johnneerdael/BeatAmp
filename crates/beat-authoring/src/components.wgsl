@group(0) @binding(1) var<storage,read> fast: array<vec4<f32>>;
@group(0) @binding(2) var<storage,read> detail: array<vec4<f32>>;
fn stereo(row: vec4<f32>, x: f32) -> f32 {
    return mix(row.z,mix(row.z,row.w,0.5+0.5*tanh(x*2.0)),control.frequency.w);
}
fn spectrum(position: f32, x: f32) -> f32 {
    let coordinate=clamp(position,0.0,1.0);
    let fi=coordinate*512.0;
    let f0=u32(floor(fi)); let f1=min(f0+1u,512u);
    let f=mix(stereo(fast[f0],x),stereo(fast[f1],x),fract(fi));
    let di=coordinate*2048.0;
    let d0=u32(floor(di)); let d1=min(d0+1u,2048u);
    let d=mix(stereo(detail[d0],x),stereo(detail[d1],x),fract(di));
    return max(f,select(0.0,d*0.85,control.windows.y>0.0));
}
// Native curved filaments. Frequency modulates amplitude as well as radiance.
fn silk(p: vec2<f32>, t: f32, phase: f32, audio: f32) -> vec3<f32> {
    let width=control.style.z/control.time.w;
    let bend=(0.18+audio*0.32)*sin(p.x*2.8+t+phase)+0.14*sin(p.x*5.3-t*0.7);
    let distance=sin((p.y-bend)*7.0+phase)/7.0;
    let stripe=luminous(distance,width*(1.0+control.energy.z*1.5));
    let focus=exp(-abs(p.y)*0.6);
    return palette(p.x*1.2+t*0.22+phase)*stripe*focus*(0.5+audio*2.0);
}
// Radial segmented crowns and broad spectral petals.
fn crown(p: vec2<f32>, t: f32, phase: f32, audio: f32) -> vec3<f32> {
    let radius=length(p); let angle=atan2(p.y,p.x);
    let lobes=5.0+floor(control.style.x*5.0);
    let ring=0.48+0.08*sin(angle*lobes+t+phase)+0.13*control.energy.z+audio*0.1;
    let width=control.style.z/control.time.w*(1.0+audio*2.0);
    let stroke=luminous(radius-ring,width)+0.4*luminous(radius-ring-0.2,width*0.7);
    let petals=pow(0.5+0.5*cos(angle*lobes-t*0.3),8.0)*exp(-abs(radius-ring)*9.0);
    return palette(angle+t*0.2+phase)*(stroke+petals*0.42)*(0.75+audio*1.6);
}
// Locally lit cell diamonds; this changes topology, not only hue.
fn lattice(p: vec2<f32>, t: f32, phase: f32, audio: f32) -> vec3<f32> {
    let q=rotation(p,0.7854+t*0.08+phase*0.1)*3.6;
    let cell=floor(q); let local=fract(q)-vec2<f32>(0.5);
    let hash=fract(sin(dot(cell,vec2<f32>(127.1,311.7))+phase)*43758.5453);
    let shape=abs(local.x)+abs(local.y)-(0.28+0.12*sin(t+hash*6.283)+audio*0.1);
    let width=control.style.z/control.time.w*3.6;
    let light=luminous(shape,width);
    let facet=pow(max(0.0,1.0-abs(local.x)-abs(local.y)),3.0);
    return palette(hash*6.283+phase+t*0.15)*(light*0.65+facet*0.3)*(0.55+audio*1.8);
}
// Interference arcs with native, antialiased contour coverage.
fn interference(p: vec2<f32>, t: f32, phase: f32, audio: f32) -> vec3<f32> {
    let offset=vec2<f32>(0.5*cos(t*0.24+phase),0.35*sin(t*0.31+phase));
    let field=length(p-offset)-length(p+offset);
    let distance=sin(field*(9.0+control.energy.z*2.0)-t+phase)/12.0;
    let contour=luminous(distance,control.style.z/control.time.w);
    let falloff=exp(-dot(p,p)*0.28);
    return palette(field*3.0+phase+t*0.13)*contour*falloff*(0.6+audio*1.8);
}
// Angular folded planes, never inferred as a particular source tunnel/fractal.
fn folding(p: vec2<f32>, t: f32, phase: f32, audio: f32) -> vec3<f32> {
    let radius=length(p); let angle=atan2(p.y,p.x)+t*0.12;
    let folds=6.0+floor(control.style.x*6.0);
    let wedge=abs(fract(angle/6.2831853*folds+0.5)-0.5);
    let edge=sin(radius*9.0-wedge*5.0-t+phase)/9.0;
    let line=luminous(edge,control.style.z/control.time.w*(1.0+audio));
    let facet=pow(1.0-wedge*2.0,3.0)*exp(-radius*0.45);
    return palette(wedge*5.0+radius+phase)*(line*0.65+facet*0.24)*(0.6+audio*1.7);
}
