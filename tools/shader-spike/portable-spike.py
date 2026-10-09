#!/usr/bin/env python3
# Compiler-route evidence only; output shaders are not published playback packages.
import pathlib,json,subprocess
BASE=pathlib.Path(__file__).resolve().parent;OUT=BASE/'output';ROOT=BASE.parents[1]
records=[]
for item in json.loads((OUT/'legacy-report.json').read_text()):
    stem=f'{item["rank"]:03}-{item["stage"]}';spv=OUT/(stem+'.spv');hlsl=OUT/(stem+'.modern.hlsl');wgsl=OUT/(stem+'.modern.wgsl');result=OUT/(stem+'.shader.json')
    cross=subprocess.run(['spirv-cross',str(spv),'--hlsl','--shader-model','50','--output',str(hlsl)],capture_output=True,text=True,timeout=20)
    slang=subprocess.run([str(BASE/'.cache/slang/bin/slangc'),'-lang','hlsl',str(hlsl),'-target','wgsl','-stage','fragment','-entry','main','-fvk-b-shift','0','0','-fvk-t-shift','1','0','-fvk-s-shift','33','0','-o',str(wgsl)],capture_output=True,text=True,timeout=20) if cross.returncode==0 else None
    validate=subprocess.run([str(ROOT/'target/debug/preset'),'validate-shader','--wgsl','--spirv',str(wgsl),'--output',str(result)],cwd=ROOT,capture_output=True,text=True,timeout=20) if slang is not None and slang.returncode==0 else None
    records.append({'rank':item['rank'],'name':item['name'],'stage':item['stage'],'portable':validate is not None and validate.returncode==0,'diagnostic':(cross.stderr+(slang.stderr if slang else '')+(validate.stderr if validate else ''))[:10000]})
(OUT/'portable-report.json').write_text(json.dumps(records,indent=2));print('portable',sum(r['portable'] for r in records),'/',len(records))
for r in records:
    if not r['portable']:print(r['rank'],r['stage'],r['diagnostic'][:350].replace('\n',' '))
