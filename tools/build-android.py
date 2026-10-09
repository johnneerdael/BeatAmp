#!/usr/bin/env python3
"""Build independent arm64 Rust + Android shell; never changes sibling sources."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
sdk = Path(os.environ.get("ANDROID_HOME", Path.home() / "Library/Android/sdk"))
ndk = sdk / "ndk/27.3.13750724"
host = "darwin-x86_64" if sys.platform == "darwin" else "linux-x86_64"
compiler = ndk / "toolchains/llvm/prebuilt" / host / "bin/aarch64-linux-android29-clang"
if not compiler.exists():
    raise SystemExit(f"Pinned Android NDK compiler missing: {compiler}")
environment = os.environ.copy()
environment["CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"] = str(compiler)
environment["CC_aarch64_linux_android"] = str(compiler)
subprocess.run(["cargo", "build", "--locked", "--release", "--target", "aarch64-linux-android", "-p", "preset-android"], cwd=root, env=environment, check=True)
# Stage the complete package catalog and original shared textures explicitly.
import json
import hashlib
assets_destination = root / "platform/android/app/src/main/assets/poc"
assets_destination.mkdir(parents=True, exist_ok=True)
catalog = json.loads((root / "reports/poc-catalog.json").read_text())
if catalog["count"] != 100 or len(catalog["presets"]) != 100:
    raise SystemExit("Complete 100-preset catalog required before Android packaging")
for row in catalog["presets"]:
    raw = (root / row["path"]).read_bytes()
    if hashlib.sha256(raw).hexdigest() != row["package_sha256"]:
        raise SystemExit("Preset package hash mismatch")
    (assets_destination / f'{row["rank"]:03}.json').write_bytes(raw)
if (root / "reports/common-audio.json").exists():
    shutil.copy2(root / "reports/common-audio.json", assets_destination / "comparison.json")
shutil.copytree(root / "corpus/textures", assets_destination / "textures", dirs_exist_ok=True)
shutil.copy2(root / "LICENSES/CC0-1.0.txt", assets_destination / "CC0-1.0.txt")
(assets_destination / "catalog.json").write_text(json.dumps(catalog, indent=2) + "\n")
destination = root / "platform/android/app/src/main/jniLibs/arm64-v8a"
destination.mkdir(parents=True, exist_ok=True)
shutil.copy2(root / "target/aarch64-linux-android/release/libpreset_android.so", destination)
(root / "platform/android/local.properties").write_text(f"sdk.dir={sdk}\n")
subprocess.run(["./gradlew", ":app:testDebugUnitTest", ":app:assembleDebug"], cwd=root / "platform/android", check=True)
print(root / "platform/android/app/build/outputs/apk/debug/app-debug.apk")
