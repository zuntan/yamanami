import sys
import chevron
from urllib.parse import parse_qs
from componentize_py_types import Ok, Err
from wit_world import exports
from wit_world.imports import types, streams
from calculator import calculate
from embedded_template import TEMPLATE

class IncomingHandler(exports.IncomingHandler):
    def handle(self, request: types.IncomingRequest, out: types.ResponseOutparam) -> None:
        path = (request.path_with_query() or "").split("?", 1)[0]
        if path not in ("/python", "/python/"):
            self.respond(out, 404, "Not found", "text/plain")
            return
        context = dict(language="Python", template="Chevron 0.14.0",
                       runtime="CPython " + sys.version.split()[0],
                       version="demo-v1", path="/python", a="", b="",
                       result="", error="", operation="")
        status = 200
        method = request.method()
        if isinstance(method, types.Method_Post):
            body = request.consume()
            stream = body.stream()
            raw = bytearray()
            while True:
                try:
                    raw.extend(stream.blocking_read(4096))
                    if len(raw) > 4096:
                        self.respond(out, 413, "Body too large", "text/plain")
                        return
                except Err as e:
                    if isinstance(e.value, streams.StreamError_Closed):
                        break
                    raise
            stream.__exit__(None, None, None)
            body.__exit__(None, None, None)
            try:
                fields = parse_qs(raw.decode("utf-8"), keep_blank_values=True, strict_parsing=True)
                if any(len(v) != 1 for v in fields.values()):
                    raise ValueError("入力が重複しています")
                context["a"] = fields.get("a", [""])[0]
                context["b"] = fields.get("b", [""])[0]
                op = fields.get("op", [""])[0]
                context["operation"] = op
                context["result"] = format(calculate(context["a"], op, context["b"]), ".12g")
            except (ValueError, UnicodeError) as e:
                context["error"] = str(e)
                status = 400
        elif not isinstance(method, types.Method_Get):
            self.respond(out, 405, "Method not allowed", "text/plain")
            return
        self.respond(out, status, chevron.render(TEMPLATE, context), "text/html; charset=utf-8")

    @staticmethod
    def respond(out, status, text, content_type):
        headers = types.Fields.from_list([("content-type", content_type.encode())])
        response = types.OutgoingResponse(headers)
        response.set_status_code(status)
        body = response.body()
        types.ResponseOutparam.set(out, Ok(response))
        stream = body.write()
        data = text.encode("utf-8")
        for i in range(0, len(data), 4096):
            stream.blocking_write_and_flush(data[i:i + 4096])
        stream.__exit__(None, None, None)
        types.OutgoingBody.finish(body, None)
