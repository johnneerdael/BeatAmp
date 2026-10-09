#!/usr/bin/env python3
"""Build the independent .beat Android route; keep historical producers unchanged."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--catalog', type=Path, default=root/'effects/beat/frozen-native-pilot/catalog.json')
args = parser.parse_args()
catalog_path = args.catalog.resolve()
catalog = json.loads(catalog_path.read_text())
if catalog.get('producer') != 'beatamp-frozen-native-pilot-v1' or catalog.get('count') != 100 or catalog.get('generated') != 100 or catalog.get('failed') != 0:
    parser.error('complete fixed100 authoring catalog required; failed outcomes cannot be silently dropped')
if catalog['corpus_manifest_sha256'] != hashlib.sha256((root/'corpus/manifest.json').read_bytes()).hexdigest():
    parser.error('frozen corpus identity mismatch')
rows = catalog['presets']
if len(rows) != 100 or {r['rank'] for r in rows} != set(range(1,101)):
    parser.error('invalid rank inventory')
verified = []
for row in rows:
    if row['path'] != f"{row['rank']:03}.beat":
        parser.error('unsafe package path')
    raw = (catalog_path.parent/row['path']).read_bytes()
    if hashlib.sha256(raw).hexdigest() != row['package_sha256']:
        parser.error('package hash mismatch: '+row['path'])
    verified.append((row['path'],raw))
sdk = Path(os.environ.get('ANDROID_HOME', Path.home()/'Library/Android/sdk'))
compiler = sdk/'ndk/27.3.13750724/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android29-clang'
if not compiler.is_file():
    parser.error('pinned NDK compiler missing: '+str(compiler))
env = os.environ.copy()
env['CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER'] = str(compiler)
env['CC_aarch64_linux_android'] = str(compiler)
env['CARGO_CACHE_AUTO_CLEAN_FREQUENCY'] = 'never'
env['JAVA_HOME'] = '/Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home'
subprocess.run(['cargo','build','--locked','--release','--target','aarch64-linux-android','-p','preset-android'],cwd=root,env=env,check=True)
destination = root/'platform/android/app/src/main/assets/beat'
destination.mkdir(parents=True,exist_ok=True)
for name,raw in verified:
    (destination/name).write_bytes(raw)
shutil.copy2(catalog_path,destination/'catalog.json')
shutil.copy2(root/'LICENSES/CC0-1.0.txt',destination/'CC0-1.0.txt')
native = root/'platform/android/app/src/main/jniLibs/arm64-v8a'
native.mkdir(parents=True,exist_ok=True)
shutil.copy2(root/'target/aarch64-linux-android/release/libpreset_android.so',native/'libpreset_android.so')
(root/'platform/android/local.properties').write_text(f'sdk.dir={sdk}\n')
subprocess.run(['./gradlew',':app:testDebugUnitTest',':app:assembleDebug'],cwd=root/'platform/android',env=env,check=True)
apk = root/'platform/android/app/build/outputs/apk/debug/app-debug.apk'
report = {'producer':'beatamp-build-v1','apk':str(apk),'apk_sha256':hashlib.sha256(apk.read_bytes()).hexdigest(),'catalog_sha256':hashlib.sha256(catalog_path.read_bytes()).hexdigest(),'packages':100,'matched_predictions':catalog['matched_predictions'],'predictions_pending':catalog['predictions_pending'],'runtime':'standalone .beat; parser/prediction are offline','native_route_intent':'native_beat=true','native_4k30':'unmeasured'}
output = root/'benchmarks/beatamp/build.json'
output.parent.mkdir(parents=True,exist_ok=True)
output.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
