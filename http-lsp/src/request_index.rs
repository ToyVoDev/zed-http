// Scans an .http document for request positions without using a full grammar.
// Each request's "anchor line" is the line containing the HTTP method + URL,
// which is what we pass to `httpyac send --line N`.

const METHODS: &[&str] = &[
    "GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "CONNECT", "TRACE",
    "GRAPHQL", "WEBSOCKET", "WS", "GRPC", "SSE", "MQTT", "AMQP",
    // WebDAV
    "PROPFIND", "PROPPATCH", "COPY", "MOVE", "LOCK", "UNLOCK", "CHECKOUT",
    "REPORT", "MERGE", "MKACTIVITY", "MKWORKSPACE", "VERSION-CONTROL",
    "LIST",
];

#[derive(Debug, Clone)]
pub struct Request {
    /// 0-indexed line number of the method/URL line.
    pub line: u32,
    pub method: String,
    pub url: String,
}

pub fn scan(text: &str) -> Vec<Request> {
    let mut out = Vec::new();
    let mut in_body = false;

    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim_start();

        if line.starts_with("###") {
            in_body = false;
            continue;
        }

        if in_body {
            continue;
        }

        if let Some((method, rest)) = line.split_once(char::is_whitespace) {
            let method_upper = method.to_ascii_uppercase();
            if METHODS.iter().any(|m| *m == method_upper) {
                let url = rest.split_whitespace().next().unwrap_or("").to_string();
                if !url.is_empty() {
                    out.push(Request {
                        line: idx as u32,
                        method: method_upper,
                        url,
                    });
                    in_body = true;
                }
            }
        }
    }

    out
}

pub fn request_at_line(requests: &[Request], line: u32, total_lines: u32) -> Option<&Request> {
    let mut last: Option<&Request> = None;
    for r in requests {
        if r.line > line {
            break;
        }
        last = Some(r);
    }
    last.filter(|_| line < total_lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_basic_requests() {
        let src = "\
@host = http://localhost
### one
GET http://example.com/a

### two
POST http://example.com/b
Content-Type: application/json

{\"k\":1}
";
        let reqs = scan(src);
        assert_eq!(reqs.len(), 2);
        assert_eq!(reqs[0].method, "GET");
        assert_eq!(reqs[0].line, 2);
        assert_eq!(reqs[1].method, "POST");
        assert_eq!(reqs[1].line, 5);
    }

    #[test]
    fn skips_method_words_in_bodies() {
        let src = "\
### one
POST http://example.com/x
Content-Type: text/plain

GET-like text that is the body
DELETE this line is also body

### two
GET http://example.com/y
";
        let reqs = scan(src);
        assert_eq!(reqs.len(), 2);
        assert_eq!(reqs[0].method, "POST");
        assert_eq!(reqs[1].method, "GET");
    }

    #[test]
    fn request_at_line_picks_enclosing_request() {
        let reqs = vec![
            Request { line: 2, method: "GET".into(), url: "/a".into() },
            Request { line: 7, method: "POST".into(), url: "/b".into() },
        ];
        assert!(request_at_line(&reqs, 0, 20).is_none(), "before any request");
        assert_eq!(request_at_line(&reqs, 2, 20).unwrap().line, 2);
        assert_eq!(request_at_line(&reqs, 5, 20).unwrap().line, 2);
        assert_eq!(request_at_line(&reqs, 7, 20).unwrap().line, 7);
        assert_eq!(request_at_line(&reqs, 19, 20).unwrap().line, 7);
    }
}
