use std::collections::BTreeMap;
use wasi::http::types::{
    Fields, IncomingRequest, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

pub fn respond(out: ResponseOutparam, status: u16, content_type: &str, text: &str) {
    let fields = Fields::new();
    fields
        .set("content-type", &[content_type.as_bytes().to_vec()])
        .unwrap();
    let response = OutgoingResponse::new(fields);
    response.set_status_code(status).unwrap();
    let body = response.body().unwrap();
    ResponseOutparam::set(out, Ok(response));
    let stream = body.write().unwrap();
    for chunk in text.as_bytes().chunks(4096) {
        stream.blocking_write_and_flush(chunk).unwrap();
    }
    drop(stream);
    OutgoingBody::finish(body, None).unwrap();
}
pub fn read_form(request: &IncomingRequest) -> Result<BTreeMap<String, String>, &'static str> {
    let body = request.consume().map_err(|_| "本文を読み込めません")?;
    let stream = body.stream().map_err(|_| "本文を読み込めません")?;
    let mut bytes = Vec::new();
    loop {
        match stream.blocking_read(4096) {
            Ok(chunk) => {
                bytes.extend(chunk);
                if bytes.len() > 4096 {
                    return Err("本文が大きすぎます");
                }
            }
            Err(wasi::io::streams::StreamError::Closed) => break,
            Err(_) => return Err("本文を読み込めません"),
        }
    }
    std::str::from_utf8(&bytes).map_err(|_| "UTF-8で入力してください")?;
    let mut fields = BTreeMap::new();
    for (key, value) in form_urlencoded::parse(&bytes) {
        if fields
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return Err("入力が重複しています");
        }
    }
    Ok(fields)
}
pub fn render(source: &str, context: &BTreeMap<&str, String>) -> String {
    let mut env = minijinja::Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::Html);
    env.add_template("calculator.html", source).unwrap();
    env.get_template("calculator.html")
        .unwrap()
        .render(context)
        .unwrap()
}
