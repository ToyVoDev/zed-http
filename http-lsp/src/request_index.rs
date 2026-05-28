// Scans an .http document for request positions without using a full grammar.
// Each request's `line` is the method/URL line that we pass to httpyac
// `send --line N`. `region` is the span from the `###` separator (or the
// previous boundary) to the line before the next `###` (or EOF), and is what
// hover/code-action/code-lens callers use to map a cursor row to a request.

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
    /// 0-indexed line number of the method/URL line — what we send to
    /// `httpyac --line`.
    pub line: u32,
    /// Inclusive start line of this request's region. This is the `###`
    /// separator line above the request (or the first method line if no `###`
    /// precedes it).
    pub region_start: u32,
    /// Inclusive end line of this request's region — the line before the
    /// next `###` separator, or the last line of the document.
    pub region_end: u32,
    pub method: String,
    pub url: String,
}

pub fn scan(text: &str) -> Vec<Request> {
    let mut out: Vec<Request> = Vec::new();
    let mut in_body = false;
    // The `###` separator line above the request currently being scanned;
    // `None` until we see the first separator (i.e. requests that appear
    // before any `###` start at their own method line).
    let mut current_separator: Option<u32> = None;
    let mut total_lines: u32 = 0;

    for (idx, raw) in text.lines().enumerate() {
        total_lines = idx as u32 + 1;
        let line = raw.trim_start();

        if line.starts_with("###") {
            // The previous request's region ends at the line before this `###`.
            if let Some(last) = out.last_mut() {
                last.region_end = (idx as u32).saturating_sub(1);
            }
            in_body = false;
            current_separator = Some(idx as u32);
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
                    let method_line = idx as u32;
                    let region_start = current_separator.unwrap_or(method_line);
                    out.push(Request {
                        line: method_line,
                        region_start,
                        // Tentative; widened when we hit the next `###` or finalized below.
                        region_end: method_line,
                        method: method_upper,
                        url,
                    });
                    in_body = true;
                    // The `###` only belongs to one request; clear so a follow-on
                    // request with no separator gets its own region starting at
                    // its method line.
                    current_separator = None;
                }
            }
        }
    }

    if let Some(last) = out.last_mut() {
        last.region_end = total_lines.saturating_sub(1);
    }

    out
}

pub fn request_at_line(requests: &[Request], line: u32) -> Option<&Request> {
    requests
        .iter()
        .find(|r| r.region_start <= line && line <= r.region_end)
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
        assert_eq!(reqs[0].region_start, 1); // the `### one` line
        assert_eq!(reqs[0].region_end, 3); // up to the blank line before `### two`
        assert_eq!(reqs[1].method, "POST");
        assert_eq!(reqs[1].line, 5);
        assert_eq!(reqs[1].region_start, 4); // the `### two` line
        assert_eq!(reqs[1].region_end, 8); // through the JSON body to EOF
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
    fn request_at_line_picks_enclosing_request_via_region() {
        // Layout:
        //   0: file-scope comment
        //   1: ### one        ← region_start of req 0
        //   2: GET /a         ← line of req 0
        //   3: blank
        //   4: ### two        ← region_start of req 1
        //   5: POST /b        ← line of req 1
        //   6: body
        //   7: body
        let reqs = vec![
            Request {
                line: 2,
                region_start: 1,
                region_end: 3,
                method: "GET".into(),
                url: "/a".into(),
            },
            Request {
                line: 5,
                region_start: 4,
                region_end: 7,
                method: "POST".into(),
                url: "/b".into(),
            },
        ];
        assert!(request_at_line(&reqs, 0).is_none(), "before any request region");
        assert_eq!(
            request_at_line(&reqs, 1).unwrap().line,
            2,
            "hovering on the `### one` line maps to the request below it",
        );
        assert_eq!(request_at_line(&reqs, 2).unwrap().line, 2);
        assert_eq!(request_at_line(&reqs, 3).unwrap().line, 2);
        assert_eq!(
            request_at_line(&reqs, 4).unwrap().line,
            5,
            "hovering on `### two` maps to its request, not the previous one",
        );
        assert_eq!(request_at_line(&reqs, 7).unwrap().line, 5);
    }
}
