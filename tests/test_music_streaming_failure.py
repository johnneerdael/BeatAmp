"""Exercise failure cleanup with isolated fake tools; never access an emulator."""
import json
import os
import pathlib
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest


class StreamingFailureTest(unittest.TestCase):
    def test_failed_capture_and_disconnected_adb_stop_all_children(self):
        repo = pathlib.Path(__file__).resolve().parents[1]
        controller = pathlib.Path(os.environ.get("PRESET_RECORD_CONTROLLER", repo / "tools/record-music-streaming.py"))
        with tempfile.TemporaryDirectory(prefix="preset-stream-failure-") as temporary:
            root = pathlib.Path(temporary)
            (root / "tools").mkdir()
            output = root / "benchmarks/music-review"
            output.mkdir(parents=True)
            shutil.copy2(controller, root / "tools/record-music-streaming.py")
            (root / "tools/record-music-chapter.py").write_text(
                "import os,pathlib,subprocess,sys\n"
                "child=subprocess.Popen([sys.executable,'-c','import time;time.sleep(300)'])\n"
                "pathlib.Path('capture-pids.json').write_text(__import__('json').dumps({'child':child.pid,'group':os.getpgrp()}))\n"
                "raise SystemExit(1)\n"
            )
            fake = root / "bin"
            fake.mkdir()
            for name, body in {
                "ffmpeg": "import os,pathlib,time\npathlib.Path('mux-pid').write_text(str(os.getpid()))\ntime.sleep(300)\n",
                "adb": "raise SystemExit(7)\n",
            }.items():
                executable = fake / name
                executable.write_text(f"#!{sys.executable}\n" + body)
                executable.chmod(0o755)
            environment = dict(os.environ, PATH=str(fake) + os.pathsep + os.environ["PATH"])
            process = subprocess.Popen([sys.executable, str(root / "tools/record-music-streaming.py"), "adapted"],
                                       cwd=root, env=environment, start_new_session=True,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                stdout, stderr = process.communicate(timeout=20)
                self.assertNotEqual(process.returncode, 0, stdout.decode())
                status = json.loads((output / "adapted-status.json").read_text())
                self.assertEqual(status["state"], "failed")
                self.assertIn("Owned app cleanup failed", status["cleanup_errors"])
                pids = json.loads((root / "capture-pids.json").read_text())
                for pid in [pids["child"], int((root / "mux-pid").read_text())]:
                    deadline = time.monotonic() + 5
                    while True:
                        try:
                            os.kill(pid, 0)
                        except ProcessLookupError:
                            break
                        self.assertLess(time.monotonic(), deadline, f"Leaked child {pid}; {stderr.decode()}")
                        time.sleep(.1)
            finally:
                # Also isolate/clean the intentionally failing pre-fix regression.
                groups = [process.pid]
                if (root / "capture-pids.json").exists():
                    groups.append(json.loads((root / "capture-pids.json").read_text())["group"])
                for group in set(groups):
                    try:
                        os.killpg(group, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                process.wait(timeout=5)
                process.stdout.close()
                process.stderr.close()


if __name__ == "__main__":
    unittest.main()
