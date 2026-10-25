use std::sync::atomic::{AtomicU32, Ordering};
use wasi::exports::http::incoming_handler::Guest;
use wasi::http::types::{
    Fields, IncomingRequest, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

struct Hello;
static CALLS: AtomicU32 = AtomicU32::new(0);

impl Guest for Hello {
    fn handle(request: IncomingRequest, out: ResponseOutparam) {
        let count = CALLS.fetch_add(1, Ordering::SeqCst) + 1;
        let path = request.path_with_query().unwrap_or_default();
        // Test endpoints intentionally exercise the host's fuel/memory/time boundaries.
        if path.ends_with("/spin") {
            loop {
                std::hint::spin_loop();
            }
        }
        if path.ends_with("/memory") {
            // Ask for 128 MiB in one instruction, exceeding the default 64 MiB limit.
            let _ = core::arch::wasm32::memory_grow::<0>(2048);
            respond(out, 500, b"memory growth was unexpectedly permitted\n");
            return;
        }
        if path.ends_with("/hold") {
            std::thread::sleep(std::time::Duration::from_millis(800));
        }
        if path.ends_with("/sleep") {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        if path.ends_with("/network") {
            use wasi::http::{
                outgoing_handler,
                types::{Method, OutgoingRequest, Scheme},
            };
            let outgoing = OutgoingRequest::new(Fields::new());
            outgoing.set_method(&Method::Get).unwrap();
            outgoing.set_scheme(Some(&Scheme::Http)).unwrap();
            outgoing.set_authority(Some("127.0.0.1:1")).unwrap();
            outgoing.set_path_with_query(Some("/")).unwrap();
            let denied = match outgoing_handler::handle(outgoing, None) {
                Err(wasi::http::types::ErrorCode::HttpRequestDenied) => true,
                Err(_) => false,
                Ok(future) => {
                    future.subscribe().block();
                    matches!(
                        future.get(),
                        Some(Ok(Err(wasi::http::types::ErrorCode::HttpRequestDenied)))
                    )
                }
            };
            respond(out, if denied { 200 } else { 500 }, b"outbound denied\n");
            return;
        }
        let mut bytes = Vec::new();
        let body = request.consume().unwrap();
        let stream = body.stream().unwrap();
        loop {
            match stream.blocking_read(4096) {
                Ok(chunk) => bytes.extend(chunk),
                Err(wasi::io::streams::StreamError::Closed) => break,
                Err(_) => {
                    respond(out, 400, b"body read failed\n");
                    return;
                }
            }
        }
        drop(stream);
        drop(body);
        let version = if cfg!(feature = "v2") { "v2" } else { "v1" };
        let header = request.headers().get("x-demo");
        let header = header
            .first()
            .map(|h| String::from_utf8_lossy(h).into_owned())
            .unwrap_or_default();
        let text = format!(
            "Hello from Yamanami {version}\nmethod={:?}\npath={path}\nx-demo={header}\ncalls={count}\nbody={}\n",
            request.method(),
            String::from_utf8_lossy(&bytes)
        );
        if path.ends_with("/large") {
            respond(out, 200, &vec![b'x'; 2 * 1024 * 1024]);
        } else {
            respond(out, 200, text.as_bytes());
        }
    }
}

fn respond(out: ResponseOutparam, status: u16, bytes: &[u8]) {
    let fields = Fields::new();
    fields
        .set("content-type", &[b"text/plain; charset=utf-8".to_vec()])
        .unwrap();
    fields.set("x-yamanami-demo", &[b"hello".to_vec()]).unwrap();
    let response = OutgoingResponse::new(fields);
    response.set_status_code(status).unwrap();
    let body = response.body().unwrap();
    ResponseOutparam::set(out, Ok(response));
    let stream = body.write().unwrap();
    for chunk in bytes.chunks(4096) {
        stream.blocking_write_and_flush(chunk).unwrap();
    }
    drop(stream);
    OutgoingBody::finish(body, None).unwrap();
}

wasi::http::proxy::export!(Hello);
