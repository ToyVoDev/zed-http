// Turns an httpyac Exchange into the text we write into the response file
// that Zed opens via window/showDocument.

use httpyac::{Exchange, RequestResult, Response};

pub enum View {
    Full,
    HeadersOnly,
}

pub fn format(exchange: &Exchange, view: View) -> String {
    let Some(req) = exchange.requests.first() else {
        return "(no request executed)\n".to_string();
    };
    format_one(req, view)
}

fn format_one(req: &RequestResult, view: View) -> String {
    let mut out = String::new();

    if let Some(name) = req.name.as_deref().or(req.title.as_deref()) {
        out.push_str(&format!("# {name}\n\n"));
    }

    match req.response.as_ref() {
        None => {
            out.push_str("(no response)\n");
            return out;
        }
        Some(resp) => write_response(&mut out, resp, view),
    }

    out
}

fn write_response(out: &mut String, resp: &Response, view: View) {
    let proto = resp.protocol.as_deref().unwrap_or("HTTP/1.1");
    let status_msg = resp.status_message.as_deref().unwrap_or("");
    out.push_str(&format!("{proto} {} {status_msg}\n", resp.status_code));

    if let Some(dur) = resp.timings.as_ref().and_then(|t| t.total) {
        let size = resp.meta.as_ref().and_then(|m| m.size.as_deref()).unwrap_or("");
        if size.is_empty() {
            out.push_str(&format!("# {dur:.0} ms\n"));
        } else {
            out.push_str(&format!("# {dur:.0} ms · {size}\n"));
        }
    }
    out.push('\n');

    let mut header_names: Vec<&String> = resp.headers.keys().collect();
    header_names.sort();
    for name in header_names {
        if let Some(value) = resp.headers.get(name) {
            let value_str = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            out.push_str(&format!("{name}: {value_str}\n"));
        }
    }

    if matches!(view, View::HeadersOnly) {
        return;
    }

    if !resp.body.is_empty() {
        out.push('\n');
        out.push_str(&resp.body);
        if !resp.body.ends_with('\n') {
            out.push('\n');
        }
    }
}
