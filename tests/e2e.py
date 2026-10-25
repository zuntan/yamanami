#!/usr/bin/env python3
"""Build and exercise the real CLI, HTTP server and WASI HTTP component.

Uses only Python's standard library. Run from any directory:
    python3 tests/e2e.py
"""
import concurrent.futures
import hashlib
import http.client
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import sqlite3
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/debug/yamanami"
TOKEN = "yamanami-local-e2e-token"
GUEST = ROOT / "examples/hello/target/wasm32-wasip2/release/yamanami_hello.wasm"


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def request(port, method, path, body=None, headers=None):
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=15)
    try:
        conn.request(method, path, body=body, headers=headers or {})
        res = conn.getresponse()
        return res.status, dict(res.getheaders()), res.read()
    finally:
        conn.close()


class Server:
    def __init__(self, directory, extra=()):
        self.directory = Path(directory)
        self.public = free_port()
        self.admin = free_port()
        while self.admin == self.public:
            self.admin = free_port()
        self.extra = extra
        self.proc = None

    def start(self):
        self.log = (self.directory / "server.log").open("ab")
        self.proc = subprocess.Popen([
            str(BINARY), "serve",
            "--listen", f"127.0.0.1:{self.public}",
            "--admin-listen", f"127.0.0.1:{self.admin}",
            "--database", str(self.directory / "apps.sqlite3"),
            *self.extra,
        ], cwd=ROOT, env={**os.environ, "YAMANAMI_TOKEN": TOKEN},
            stdout=self.log, stderr=self.log)
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if self.proc.poll() is not None:
                raise AssertionError((self.directory / "server.log").read_text())
            try:
                if self.api("GET", "/v1/apps")[0] == 200:
                    return self
            except OSError:
                pass
            time.sleep(0.05)
        self.stop()
        raise AssertionError("server did not become ready")

    def stop(self):
        if self.proc is not None:
            if self.proc.poll() is None:
                self.proc.send_signal(signal.SIGINT)
                try:
                    self.proc.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    self.proc.kill()
                    self.proc.wait()
            self.log.close()
            self.proc = None

    def api(self, method, path, body=None, headers=None):
        return request(self.admin, method, path, body, {
            "Authorization": f"Bearer {TOKEN}", **(headers or {})})

    def deploy(self, name, mount, wasm):
        return self.api("PUT", f"/v1/apps/{name}", wasm,
                        {"x-yamanami-path": mount, "Content-Type": "application/wasm"})

    def cli(self, *args, ok=True):
        result = subprocess.run([
            str(BINARY), "ctl", "--endpoint", f"http://127.0.0.1:{self.admin}", *args
        ], env={**os.environ, "YAMANAMI_TOKEN": TOKEN}, cwd=ROOT,
            capture_output=True, text=True, timeout=120)
        if ok and result.returncode:
            raise AssertionError(result.stderr)
        return result


class EndToEnd(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.artifacts = tempfile.TemporaryDirectory(prefix="yamanami-guests-")
        cls.v1_path = Path(cls.artifacts.name) / "hello-v1.wasm"
        cls.v2_path = Path(cls.artifacts.name) / "hello-v2.wasm"
        for features, destination in [(None, cls.v1_path), ("v2", cls.v2_path)]:
            command = ["cargo", "build", "--locked", "--manifest-path",
                       str(ROOT / "examples/hello/Cargo.toml"),
                       "--target", "wasm32-wasip2", "--release"]
            if features:
                command += ["--features", features]
            subprocess.run(command, cwd=ROOT, check=True)
            shutil.copyfile(GUEST, destination)
        cls.v1 = cls.v1_path.read_bytes()
        cls.v2 = cls.v2_path.read_bytes()

    @classmethod
    def tearDownClass(cls):
        cls.artifacts.cleanup()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="yamanami-e2e-")
        self.server = Server(self.temp.name).start()

    def tearDown(self):
        self.server.stop()
        self.temp.cleanup()

    def test_cli_deploy_http_update_restart_and_dedup(self):
        deployed = json.loads(self.server.cli(
            "deploy", str(self.v1_path), "--app", "hello", "--path", "/hello").stdout)
        self.assertEqual(deployed["sha256"], hashlib.sha256(self.v1).hexdigest())
        self.assertEqual(len(json.loads(self.server.cli("list").stdout)), 1)
        self.assertEqual(json.loads(self.server.cli("info", "hello").stdout)["release"],
                         deployed["release"])
        status, headers, body = request(self.server.public, "POST", "/hello/echo?q=1",
                                        b"request body", {"x-demo": "sample"})
        self.assertEqual(status, 200, body)
        self.assertEqual(headers["x-yamanami-demo"], "hello")
        for expected in [b"Yamanami v1", b"method=Method::Post", b"path=/hello/echo?q=1",
                         b"x-demo=sample", b"body=request body", b"calls=1"]:
            self.assertIn(expected, body)
        self.assertEqual(request(self.server.public, "GET", "/helloworld")[0], 404)
        self.assertEqual(self.server.deploy("other", "/other", self.v1)[0], 201)
        self.assertIn(b"calls=1", request(self.server.public, "GET", "/other")[2])
        with sqlite3.connect(Path(self.temp.name) / "apps.sqlite3") as db:
            self.assertEqual(db.execute("SELECT count(*) FROM modules").fetchone()[0], 1)
        # New requests select the new release; invalid updates preserve it.
        updated = json.loads(self.server.cli(
            "deploy", str(self.v2_path), "--app", "hello", "--path", "/hello").stdout)
        self.assertNotEqual(updated["release"], deployed["release"])
        # Both versions are now cached. Switch while an old request is suspended.
        self.assertEqual(self.server.deploy("hello", "/hello", self.v1)[0], 201)
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            old_request = pool.submit(request, self.server.public, "GET", "/hello/hold")
            time.sleep(0.1)
            self.assertEqual(self.server.deploy("hello", "/hello", self.v2)[0], 201)
            self.assertFalse(old_request.done(), "old request should still be executing")
            self.assertIn(b"Yamanami v2", request(self.server.public, "GET", "/hello")[2])
            self.assertIn(b"Yamanami v1", old_request.result()[2])
        self.assertEqual(self.server.deploy("hello", "/hello", b"not a component")[0], 422)
        self.assertEqual(self.server.deploy("other", "/hello", self.v2)[0], 409)
        self.assertIn(b"Yamanami v2", request(self.server.public, "GET", "/hello")[2])
        self.server.stop()
        self.server.start()
        self.assertIn(b"Yamanami v2", request(self.server.public, "GET", "/hello")[2])
        self.assertIn(b"Yamanami v1", request(self.server.public, "GET", "/other")[2])
        self.assertEqual(len(json.loads(self.server.cli("list").stdout)), 2)

    def test_parallel_requests_and_fresh_memory(self):
        self.assertEqual(self.server.deploy("hello", "/hello", self.v1)[0], 201)
        self.assertEqual(self.server.deploy("other", "/other", self.v1)[0], 201)
        # Each /sleep yields inside a WASI host call. Four requests should overlap.
        started = time.monotonic()
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            results = list(pool.map(
                lambda i: request(self.server.public, "GET",
                                  "/hello/sleep" if i % 2 else "/other/sleep"), range(4)))
        elapsed = time.monotonic() - started
        for status, _, body in results:
            self.assertEqual(status, 200, body)
            self.assertIn(b"calls=1", body)
        self.assertLess(elapsed, 0.9, f"requests appear serialized ({elapsed:.3f}s)")
        print(f"\n4 overlapping 250ms WASI waits: {elapsed:.3f}s")

    def test_auth_separation_network_and_resource_limits(self):
        self.assertEqual(request(self.server.admin, "GET", "/v1/apps")[0], 401)
        self.assertEqual(request(self.server.admin, "GET", "/v1/apps",
                                 headers={"Authorization": "Bearer wrong"})[0], 401)
        self.assertEqual(request(self.server.public, "PUT", "/v1/apps/bad", self.v1)[0], 404)
        self.assertEqual(self.server.deploy("hello", "/hello", self.v1)[0], 201)
        for endpoint in ["spin", "memory", "large"]:
            started = time.monotonic()
            status, _, body = request(self.server.public, "GET", f"/hello/{endpoint}")
            self.assertEqual(status, 502, (endpoint, body))
            self.assertLess(time.monotonic() - started, 6)
            self.assertEqual(request(self.server.public, "GET", "/hello")[0], 200)
        self.assertEqual(request(self.server.public, "GET", "/hello/network")[0], 200)
        self.assertEqual(request(self.server.public, "POST", "/hello", b"x" * (1024 * 1024 + 1))[0], 413)
        self.assertEqual(self.server.deploy("big", "/big", b"x" * (16 * 1024 * 1024 + 1))[0], 413)

    def test_deadline_and_admission(self):
        self.server.stop()
        self.server = Server(self.temp.name, [
            "--app-concurrency", "1", "--timeout-ms", "100",
        ]).start()
        self.assertEqual(self.server.deploy("hello", "/hello", self.v1)[0], 201)
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            first = pool.submit(request, self.server.public, "GET", "/hello/sleep")
            time.sleep(0.04)
            second = request(self.server.public, "GET", "/hello")
            self.assertEqual(second[0], 503)
            self.assertEqual(first.result()[0], 502)
        self.assertEqual(request(self.server.public, "GET", "/hello")[0], 200)

    def test_database_cannot_be_shared_by_two_servers(self):
        result = subprocess.run([
            str(BINARY), "serve", "--listen", f"127.0.0.1:{free_port()}",
            "--admin-listen", f"127.0.0.1:{free_port()}",
            "--database", str(Path(self.temp.name) / "apps.sqlite3"),
        ], cwd=ROOT, env={**os.environ, "YAMANAMI_TOKEN": TOKEN},
            capture_output=True, timeout=5)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"already in use", result.stderr)

    def test_bad_configuration_and_cli_errors(self):
        env = {**os.environ, "YAMANAMI_TOKEN": TOKEN}
        for args in [
            ["--admin-listen", "0.0.0.0:9090"],
            ["--listen", "0.0.0.0:8080"],
            ["--concurrency", "0"],
            ["--timeout-ms", "0"],
        ]:
            result = subprocess.run([str(BINARY), "serve", *args], cwd=ROOT,
                                    env=env, capture_output=True, timeout=5)
            self.assertNotEqual(result.returncode, 0)
        self.assertNotEqual(self.server.cli("info", "missing", ok=False).returncode, 0)


if __name__ == "__main__":
    subprocess.run(["cargo", "build", "--locked", "-j", "2"], cwd=ROOT, check=True)
    unittest.main(verbosity=2)
