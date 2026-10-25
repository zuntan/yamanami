#!/usr/bin/env python3
"""Exercise the container entrypoint with the real server, without a Docker daemon."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import sqlite3
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("host", ROOT / "tests/e2e.py")
host = importlib.util.module_from_spec(spec)
spec.loader.exec_module(host)
APPS = ["index", "calculator-rust", "calculator-cpp", "calculator-python", "calculator-typescript"]


class Entrypoint(unittest.TestCase):
    def test_bootstrap_restart_and_changed_binary(self):
        with tempfile.TemporaryDirectory(prefix="yamanami-container-entrypoint-") as tmp:
            directory = Path(tmp)
            apps = directory / "apps"
            apps.mkdir()
            original = host.GUEST.read_bytes()
            for name in APPS:
                (apps / f"{name}.wasm").write_bytes(original)
            admin, public = host.free_port(), host.free_port()
            while admin == public:
                public = host.free_port()
            env = {**os.environ,
                   "YAMANAMI_DATA_DIR": str(directory),
                   "YAMANAMI_BIN": str(host.BINARY),
                   "YAMANAMI_ADMIN_ENDPOINT": f"http://127.0.0.1:{admin}",
                   "YAMANAMI_APPS_DIR": str(apps),
                   "YAMANAMI_DEPLOY_DEMO": "1",
                   "RAYON_NUM_THREADS": "2"}
            env.pop("YAMANAMI_TOKEN", None)
            command = ["sh", str(ROOT / "docker/entrypoint.sh"), "serve",
                       "--database", str(directory / "apps.sqlite3"),
                       "--listen", f"127.0.0.1:{public}",
                       "--admin-listen", f"127.0.0.1:{admin}", "--compiler-opt-level", "none"]
            log = (directory / "entrypoint.log").open("ab")

            def start():
                process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log, stderr=log)
                deadline = time.monotonic() + 60
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise AssertionError((directory / "entrypoint.log").read_text())
                    if (directory / "demo-ready").exists():
                        result = subprocess.run(["sh", str(ROOT / "docker/entrypoint.sh"), "health"],
                                                cwd=ROOT, env=env, capture_output=True, timeout=5)
                        if result.returncode == 0:
                            return process
                    time.sleep(.1)
                process.terminate()
                process.wait(timeout=10)
                raise AssertionError("entrypoint readiness deadline exceeded")

            def stop(process):
                process.send_signal(signal.SIGTERM)
                process.wait(timeout=10)

            def release_count():
                with sqlite3.connect(directory / "apps.sqlite3") as db:
                    return db.execute("SELECT count(*) FROM releases").fetchone()[0]

            process = start()
            try:
                self.assertEqual(release_count(), 5)
                token = (directory / "admin-token").read_text()
                self.assertEqual(len(token), 64)
                self.assertEqual((directory / "admin-token").stat().st_mode & 0o777, 0o600)
                listed = subprocess.run(["sh", str(ROOT / "docker/entrypoint.sh"), "ctl", "list"],
                                        cwd=ROOT, env=env, capture_output=True, check=True)
                self.assertEqual(len(json.loads(listed.stdout)), 5)
            finally:
                stop(process)
            process = start()
            try:
                self.assertEqual(release_count(), 5, "unchanged restart created unnecessary releases")
                self.assertEqual((directory / "admin-token").read_text(), token)
            finally:
                stop(process)
            # A valid custom section changes only the binary hash, not the HTTP interface.
            (apps / "calculator-rust.wasm").write_bytes(original + bytes([0, 3, 1, ord("u"), ord("1")]))
            process = start()
            try:
                self.assertEqual(release_count(), 6, "only the changed sample should be redeployed")
            finally:
                stop(process)
                log.close()


if __name__ == "__main__":
    subprocess.run(["cargo", "build", "--locked"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "build", "--locked", "--manifest-path", "examples/hello/Cargo.toml",
                    "--target", "wasm32-wasip2", "--release"], cwd=ROOT, check=True)
    unittest.main(verbosity=2)
