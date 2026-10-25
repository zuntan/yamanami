pub fn calculate(a: &str, op: &str, b: &str) -> Result<f64, &'static str> {
    fn number(s: &str) -> Result<f64, &'static str> {
        if s.is_empty()
            || s.len() > 100
            || s.bytes()
                .any(|b| !b.is_ascii_digit() && !b"+-.eE".contains(&b))
        {
            return Err("数値を入力してください");
        }
        let n: f64 = s.parse().map_err(|_| "数値を入力してください")?;
        if !n.is_finite() {
            return Err("有限の数値を入力してください");
        }
        Ok(n)
    }
    let (a, b) = (number(a)?, number(b)?);
    let result = match op {
        "add" => a + b,
        "sub" => a - b,
        "mul" => a * b,
        "div" if b == 0.0 => return Err("ゼロで除算できません"),
        "div" => a / b,
        _ => return Err("不明な演算子です"),
    };
    if !result.is_finite() {
        return Err("結果が数値の範囲を超えています");
    }
    Ok(result)
}
#[cfg(target_arch = "wasm32")]
mod http {
    use super::calculate;
    use std::collections::BTreeMap;
    use wasi::{
        exports::http::incoming_handler::Guest,
        http::types::{IncomingRequest, Method, ResponseOutparam},
    };
    use yamanami_demo_http::{read_form, render, respond};
    struct Calculator;
    impl Guest for Calculator {
        fn handle(request: IncomingRequest, out: ResponseOutparam) {
            let path = request.path_with_query().unwrap_or_default();
            if !matches!(path.split('?').next(), Some("/rust" | "/rust/")) {
                respond(out, 404, "text/plain", "Not found");
                return;
            }
            let mut context: BTreeMap<&str, String> = [
                ("language", "Rust"),
                ("template", "MiniJinja 2.24.0"),
                ("runtime", "Rust / wasm32-wasip2"),
                ("path", "/rust"),
                (
                    "version",
                    if cfg!(feature = "v2") {
                        "demo-v2"
                    } else {
                        "demo-v1"
                    },
                ),
                ("a", ""),
                ("b", ""),
                ("operation", ""),
                ("result", ""),
                ("error", ""),
            ]
            .into_iter()
            .map(|(k, v)| (k, v.into()))
            .collect();
            let mut status = 200;
            match request.method() {
                Method::Post => {
                    let result = read_form(&request).and_then(|fields| {
                        for key in ["a", "b"] {
                            context.insert(key, fields.get(key).cloned().unwrap_or_default());
                        }
                        let op = fields.get("op").cloned().unwrap_or_default();
                        context.insert("operation", op.clone());
                        calculate(&context["a"], &op, &context["b"])
                    });
                    match result {
                        Ok(value) => {
                            context.insert("result", value.to_string());
                        }
                        Err(message) => {
                            context.insert("error", message.into());
                            status = 400;
                        }
                    }
                }
                Method::Get => {}
                _ => {
                    respond(out, 405, "text/plain", "Method not allowed");
                    return;
                }
            }
            let html = render(include_str!("../../shared/calculator.html"), &context);
            respond(out, status, "text/html; charset=utf-8", &html);
        }
    }
    wasi::http::proxy::export!(Calculator);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arithmetic() {
        for (a, op, b, result) in [
            ("2", "add", "3", 5.),
            ("7", "sub", "9", -2.),
            ("-2.5", "mul", "4", -10.),
            ("9", "div", "4", 2.25),
        ] {
            assert_eq!(calculate(a, op, b), Ok(result));
        }
    }
    #[test]
    fn errors() {
        for (a, op, b) in [
            ("1", "div", "0"),
            ("1", "div", "-0"),
            ("NaN", "add", "1"),
            ("1e309", "add", "1"),
            ("", "add", "2"),
            ("1x", "add", "2"),
            ("1", "eval", "2"),
            ("1e308", "mul", "1e308"),
        ] {
            assert!(calculate(a, op, b).is_err());
        }
    }
}
