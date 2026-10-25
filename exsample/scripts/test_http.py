#!/usr/bin/env python3
"""Real HTTP tests. Build all demo artifacts before running this script."""
import concurrent.futures
import html
from html.parser import HTMLParser
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import time
import unittest
from urllib.parse import urlencode

os.environ.setdefault("RAYON_NUM_THREADS", "4")
ROOT = Path(__file__).resolve().parents[2]
DEMO = ROOT / "exsample"
spec = importlib.util.spec_from_file_location("host_tests", ROOT / "tests/e2e.py")
host = importlib.util.module_from_spec(spec)
spec.loader.exec_module(host)
from deploy import APPLICATIONS

OPTIONS = [
    "--compiler-opt-level", "none", "--upload-limit", "33554432",
    "--memory-mib", "256", "--max-memories", "8", "--max-instances", "128",
    "--max-tables", "32", "--fuel", "1000000000", "--timeout-ms", "10000",
    "--concurrency", "4", "--app-concurrency", "1",
]


class DemoServer(host.Server):
    def start(self):
        self.log = (self.directory / "server.log").open("ab")
        self.proc = subprocess.Popen([
            str(host.BINARY), "serve", "--listen", f"127.0.0.1:{self.public}",
            "--admin-listen", f"127.0.0.1:{self.admin}",
            "--database", str(self.directory / "apps.sqlite3"), *OPTIONS
        ], cwd=ROOT, env={**os.environ, "YAMANAMI_TOKEN": host.TOKEN},
            stdout=self.log, stderr=self.log)
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            if self.proc.poll() is not None:
                raise AssertionError((self.directory / "server.log").read_text())
            try:
                if self.api("GET", "/v1/apps")[0] == 200:
                    return self
            except OSError:
                pass
            time.sleep(.1)
        self.stop()
        raise AssertionError("server startup deadline exceeded")


class Frames(HTMLParser):
    def __init__(self):
        super().__init__()
        self.frames = []
    def handle_starttag(self, tag, attrs):
        if tag == "iframe":
            self.frames.append(dict(attrs).get("src"))


class Polyglot(unittest.TestCase):
    def test_full_demo(self):
        with tempfile.TemporaryDirectory(prefix="yamanami-polyglot-") as directory:
            server = DemoServer(directory).start()
            try:
                for app, path in APPLICATIONS:
                    print(f"Deploying {app}", flush=True)
                    result = server.cli("deploy", str(DEMO / f"build/{app}.wasm"),
                                        "--app", app, "--path", path)
                    self.assertEqual(json.loads(result.stdout)["name"], app)
                infos = {app: json.loads(server.cli("info", app).stdout) for app, _ in APPLICATIONS}
                self.assertEqual(len(json.loads(server.cli("list").stdout)), 5)
                status, headers, body = host.request(server.public, "GET", "/")
                self.assertEqual(status, 200)
                self.assertIn("text/html", headers["content-type"])
                frames = Frames()
                frames.feed(body.decode())
                self.assertEqual(frames.frames, ["/rust/", "/cpp/", "/python/", "/typescript/"])
                for language in ["rust", "cpp", "python", "typescript"]:
                    with self.subTest(language=language):
                        path = f"/{language}/"
                        status, _, body = host.request(server.public, "GET", path)
                        self.assertEqual(status, 200, body)
                        self.assertIn(b'<form method="post"', body)
                        self.assertNotIn(b"<script", body)  # The demo does no browser-side arithmetic.
                        for a, op, b, expected in [
                            ("2","add","3",5),("7","sub","9",-2),
                            ("-2.5","mul","4",-10),("9","div","4",2.25),
                            ("0.1","add","0.2",.3),("1e2","div","4",25),
                        ]:
                            status, _, body = host.request(
                                server.public, "POST", path, urlencode(dict(a=a,op=op,b=b)),
                                {"content-type":"application/x-www-form-urlencoded"})
                            self.assertEqual(status, 200, body)
                            value = re.search(rb'<output id="result">([^<]*)</output>', body)
                            self.assertIsNotNone(value, body)
                            self.assertAlmostEqual(float(value.group(1)), expected)
                        for a, op, b in [("1","div","0"),("1","div","-0"),
                                          ("NaN","add","1"),("1e309","add","1"),
                                          ("1","eval","2"),("1e308","mul","1e308")]:
                            status, _, body = host.request(server.public, "POST", path,
                                urlencode(dict(a=a,op=op,b=b)),
                                {"content-type":"application/x-www-form-urlencoded"})
                            self.assertEqual(status, 400, body)
                        attack = '<img src=x onerror="alert(1)">'
                        status, _, body = host.request(server.public, "POST", path,
                            urlencode(dict(a=attack,op="add",b="1")),
                            {"content-type":"application/x-www-form-urlencoded"})
                        self.assertEqual(status, 400)
                        self.assertNotIn(attack.encode(), body)
                        self.assertIn(b"&lt;img", body)
                        cleared = host.request(server.public, "GET", path)[2]
                        self.assertIn(b'<output id="result"></output>', cleared)
                        self.assertEqual(host.request(server.public, "GET", path+"unknown")[0], 404)
                with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
                    responses = list(pool.map(lambda language: host.request(
                        server.public, "POST", f"/{language}/", "a=6&op=mul&b=7",
                        {"content-type":"application/x-www-form-urlencoded"}),
                        ["rust", "cpp", "python", "typescript"]))
                self.assertTrue(all(status == 200 and b'>42</output>' in body for status, _, body in responses))
                subprocess.run(["node", str(DEMO / "scripts/test_browser.mjs"),
                                f"http://127.0.0.1:{server.public}"], cwd=ROOT, check=True)
                subprocess.run(["python3", str(DEMO / "scripts/build.py"), "--language", "rust",
                                "--rust-v2"], cwd=ROOT, check=True)
                server.cli("deploy", str(DEMO / "build/calculator-rust.wasm"),
                           "--app", "calculator-rust", "--path", "/rust")
                self.assertIn(b"demo-v2", host.request(server.public, "GET", "/rust/")[2])
                for app, _ in APPLICATIONS:
                    if app != "calculator-rust":
                        self.assertEqual(json.loads(server.cli("info", app).stdout)["sha256"],
                                         infos[app]["sha256"])
                print("Restarting with all five persisted applications", flush=True)
                server.stop()
                server.start()
                for language in ["rust","cpp","python","typescript"]:
                    self.assertEqual(host.request(server.public, "GET", f"/{language}/")[0], 200)
                self.assertIn(b"demo-v2", host.request(server.public, "GET", "/rust/")[2])
            finally:
                server.stop()


if __name__ == "__main__":
    unittest.main(verbosity=2)
