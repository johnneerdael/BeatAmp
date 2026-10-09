#!/usr/bin/env python3
# Offline feasibility probe. Does not publish playable packages.
import json,re,subprocess,pathlib
ROOT=pathlib.Path(__file__).resolve().parents[2]
SLANG=ROOT/'tools/shader-spike/.cache/slang/bin/slangc'
OUT=ROOT/'tools/shader-spike/output'

def source(body):
    code='\n'.join(text for line,text in body)
    declared=set(re.findall(r'\b(?:sampler|sampler2D|sampler3D)\s+(sampler_\w+)',code))
    names=sorted(set(re.findall(r'\bsampler_[A-Za-z0-9_]+',code))|{'sampler_main','sampler_blur1','sampler_blur2','sampler_blur3'})
    sizes=sorted(set(re.findall(r'\btexsize_[A-Za-z0-9_]+',code)))
    head='struct MilkParams { float4 data[64]; };\nConstantBuffer<MilkParams> globals;\n'
    for name in names:
        if name in declared:continue
        dim='3D' if 'noisevol_' in name else '2D'
        head+=f'Sampler{dim}<float4> {name};\n'
    macros={'time':'globals.data[0].x','fps':'globals.data[0].y','frame':'globals.data[0].z','progress':'globals.data[0].w','bass':'globals.data[1].x','mid':'globals.data[1].y','treb':'globals.data[1].z','vol':'globals.data[1].w','bass_att':'globals.data[2].x','mid_att':'globals.data[2].y','treb_att':'globals.data[2].z','vol_att':'globals.data[2].w','texsize':'globals.data[3]','aspect':'globals.data[4]','rand_frame':'globals.data[5]','rand_preset':'globals.data[6]','hue_shader':'globals.data[7].xyz','blur1_min':'globals.data[16].x','blur1_max':'globals.data[16].y','blur2_min':'globals.data[16].z','blur2_max':'globals.data[16].w','blur3_min':'globals.data[17].x','blur3_max':'globals.data[17].y'}
    for i in range(1,33):macros[f'q{i}']=f'globals.data[{8+(i-1)//4}].{"xyzw"[(i-1)%4]}'
    for i,name in enumerate(sizes):macros[name]=f'globals.data[{32+i}]'
    for name,value in macros.items():head+=f'#define {name} ({value})\n'
    head+='''#define sampler Sampler2D<float4>
#define sampler2D Sampler2D<float4>
#define sampler3D Sampler3D<float4>
#define tex2D(t,u) ((t).Sample(u))
#define tex2d(t,u) ((t).Sample(u))
#define tex3D(t,u) ((t).Sample(u))
#define tex2Dbias(t,u) ((t).SampleBias((u).xy,(u).w))
float3 GetPixel(float2 uv) { return sampler_main.Sample(uv).xyz; }
float lum(float3 v) { return dot(v,float3(.32,.49,.29)); }
#define M_PI 3.141592653589793
#define M_PI_2 6.283185307179586
#define M_INV_PI_2 .1591549430918953
#define roam_cos cos(time*float4(.17,.13,.11,.07))
#define roam_sin sin(time*float4(.17,.13,.11,.07))
float3 GetBlur1(float2 uv) { return sampler_blur1.Sample(uv).xyz; }
float3 GetBlur2(float2 uv) { return sampler_blur2.Sample(uv).xyz; }
float3 GetBlur3(float2 uv) { return sampler_blur3.Sample(uv).xyz; }
#define shader_body void milk_body(float2 uv,float2 uv_orig,float rad,float ang,out float3 ret)
'''
    for i in range(1,5):head+=f'#define rot_d{i} float4x3(1,0,0,0,1,0,0,0,1,0,0,0)\n'
    head+=f'#line {body[0][0]} "source.milk"\n'
    head+='\n'.join(text for _,text in body)+'\n'
    head+='''\n#line 1 "adapter.slang"
float4 fragmentMain(float4 position:SV_Position,float2 coord:TEXCOORD0,float2 warped:TEXCOORD1):SV_Target {
    float2 p=(coord-.5)*2*aspect.xy; float rad=length(p); float ang=atan2(p.y,p.x);
    float3 ret=0; milk_body(warped,coord,rad,ang,ret); return float4(ret,1);
}
'''
    return head

def run():
    manifest=json.loads((ROOT/'corpus/manifest.json').read_text());records=[]
    for p in manifest['presets']:
        text=(ROOT/p['copied_path']).read_text(errors='replace');sections={'warp':[],'comp':[]}
        for line,l in enumerate(text.splitlines(),1):
            m=re.match(r'(warp|comp)_([0-9]+)=(.*)',l)
            if m:sections[m[1]].append((int(m[2]),line,m[3].removeprefix('`')))
        for stage,items in sections.items():
            if not items:continue
            body=[(line,s) for _,line,s in sorted(items)]; path=OUT/f'{p["selection_rank"]:03}-{stage}.slang';path.write_text(source(body))
            result=subprocess.run([str(SLANG),'-lang','hlsl',str(path),'-target','wgsl','-stage','fragment','-entry','fragmentMain','-o',str(path.with_suffix('.wgsl'))],capture_output=True,text=True,timeout=20)
            records.append({'rank':p['selection_rank'],'name':p['path'],'stage':stage,'compiled':result.returncode==0,'diagnostic':result.stderr})
    (OUT/'report.json').write_text(json.dumps(records,indent=2));print('compiled',sum(r['compiled'] for r in records),'/',len(records))
    for r in records:
        if not r['compiled']:print(r['rank'],r['stage'],r['diagnostic'][:350].replace('\n',' '))
if __name__=='__main__':run()
