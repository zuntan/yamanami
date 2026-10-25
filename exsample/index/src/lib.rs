#[cfg(target_arch = "wasm32")]
mod http {
    use std::collections::BTreeMap;
    use wasi::{
        exports::http::incoming_handler::Guest,
        http::types::{IncomingRequest, Method, ResponseOutparam},
    };
    use yamanami_demo_http::{render, respond};
    struct Index;
    impl Guest for Index {
        fn handle(request: IncomingRequest, out: ResponseOutparam) {
            if request
                .path_with_query()
                .unwrap_or_default()
                .split('?')
                .next()
                != Some("/")
            {
                respond(out, 404, "text/plain", "Not found");
                return;
            }
            if !matches!(request.method(), Method::Get) {
                respond(out, 405, "text/plain", "Method not allowed");
                return;
            }
            let html = render(include_str!("../../shared/index.html"), &BTreeMap::new());
            respond(out, 200, "text/html; charset=utf-8", &html);
        }
    }
    wasi::http::proxy::export!(Index);
}
