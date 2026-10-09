#!/usr/bin/env python3
import importlib.util,pathlib,re,subprocess,json
BASE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('spike',BASE/'spike.py');spike=importlib.util.module_from_spec(spec);spec.loader.exec_module(spike)
OUT=BASE/'output';ROOT=BASE.parents[1]

def legacy_source(body):
    source=spike.source(body)
    source=source.replace('struct MilkParams { float4 data[64]; };\nConstantBuffer<MilkParams> globals;', 'cbuffer MilkParams { float4 _params[64]; };').replace('globals.data','_params')
    source=re.sub(r'Sampler(2D|3D)<float4>',r'sampler\1',source)
    source=source.replace('#define sampler sampler2D','#define sampler sampler2D')
    source=re.sub(r'#define tex2D.*\n|#define tex2d.*\n|#define tex3D.*\n|#define tex2Dbias.*\n','',source)
    source=re.sub(r'(sampler_[A-Za-z0-9_]+)\.Sample\((uv)\)',r'tex2D(\1,\2)',source)
    source=source.replace('#define sampler2D sampler2D\n','').replace('#define sampler3D sampler3D\n','')
    source=source.replace('SV_Target','COLOR0')
    source='#define tex2d tex2D\n'+source
    source=re.sub(r'(?m)^(\s*(?:sampler|sampler2D|sampler3D)\s+sampler_\w+\s*;)' ,r'uniform \1',source)
    source=source.replace('#define M_PI 3.141592653589793','#define M_PI 3.141592653589793\n#define slow_roam_sin sin(time*float4(.005,.008,.013,.022))\n#define slow_roam_cos cos(time*float4(.005,.008,.013,.022))')
    # Existing texture-size declarations duplicate the provided ABI macros. Probe only.
    source=re.sub(r'float4\s+texsize_\w+\s*;','',source)
    source='#define _qa (_params[8])\n#define _qb (_params[9])\nbool all(bool4 v) { return v.x && v.y && v.z && v.w; }\nbool all(bool3 v) { return v.x && v.y && v.z; }\nbool all(bool2 v) { return v.x && v.y; }\n'+source
    original='\n'.join(s for _,s in body)
    writes=set(re.findall(r'\b(q[0-9]+|texsize)(?:\s*\.\s*[xyzw]+)?\s*(?:=(?!=)|[+*/-]=)',original))
    for name in writes:
        match=re.search(r'^#define '+name+r' (.+)$',source,re.M)
        if match:
            value=match[1];source=source[:match.start()]+('static float4 ' if name=='texsize' else 'static float ')+name+';'+source[match.end():]
            source=source.replace('    float2 p=',f'    {name}={value};\n    float2 p=')
    return source

records=[];m=json.loads((ROOT/'corpus/manifest.json').read_text())
for p in m['presets']:
    sections={'warp':[],'comp':[]}
    for line,l in enumerate((ROOT/p['copied_path']).read_text(errors='replace').splitlines(),1):
        match=re.match(r'(warp|comp)_([0-9]+)=(.*)',l)
        if match:sections[match[1]].append((int(match[2]),line,match[3].removeprefix('`')))
    for stage,items in sections.items():
        if not items:continue
        path=OUT/f'{p["selection_rank"]:03}-{stage}.hlsl';path.write_text(legacy_source([(line,s) for _,line,s in sorted(items)]))
        preprocessed=path.with_suffix('.pp.hlsl')
        subprocess.run(['clang','-x','c','-E','-P',str(path),'-o',str(preprocessed)],capture_output=True,text=True,check=True,timeout=20)
        pp=preprocessed.read_text();seen=set();clean=[]
        for line in pp.splitlines():
            if re.match(r'\s*uniform sampler[23]D\s+sampler_\w+\s*;',line):
                if line.strip() in seen:continue
                seen.add(line.strip())
            clean.append(line)
        preprocessed.write_text('\n'.join(clean)+'\n')
        parse=subprocess.run([str(BASE/'.cache/legacy/legacy-hlsl'),str(preprocessed),str(path.with_suffix('.glsl'))],capture_output=True,text=True,timeout=20)
        compile=None
        if parse.returncode==0:
            glsl=path.with_suffix('.glsl');g=glsl.read_text();g=g.replace('out vec4 rast_FragData[1];','layout(location=0) out vec4 rast_Color0;').replace('rast_FragData[0]','rast_Color0');glsl.write_text(g)
            compile=subprocess.run(['glslangValidator','-V','--auto-map-bindings','--auto-map-locations','-S','frag',str(path.with_suffix('.glsl')),'-o',str(path.with_suffix('.spv'))],capture_output=True,text=True,timeout=20)
        records.append({'rank':p['selection_rank'],'name':p['path'],'stage':stage,'parsed':parse.returncode==0,'compiled':compile is not None and compile.returncode==0,'diagnostic':parse.stdout+parse.stderr+(compile.stdout+compile.stderr if compile else '')})
(OUT/'legacy-report.json').write_text(json.dumps(records,indent=2));print('parsed',sum(r['parsed'] for r in records),'compiled',sum(r['compiled'] for r in records),'/',len(records))
for r in records:
    if not r['compiled']:print(r['rank'],r['stage'],r['diagnostic'][:300].replace('\n',' '))
