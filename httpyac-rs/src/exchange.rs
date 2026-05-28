//! Typed slice of httpyac's `--json --output exchange` payload.
//!
//! Only the fields we currently consume are modeled; httpyac emits a much
//! larger structure but unmentioned fields are silently ignored.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Exchange {
    pub requests: Vec<RequestResult>,
    pub summary: Summary,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Summary {
    #[serde(rename = "totalRequests", default)]
    pub total_requests: u32,
    #[serde(rename = "failedRequests", default)]
    pub failed_requests: u32,
    #[serde(rename = "erroredRequests", default)]
    pub errored_requests: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestResult {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    /// 0-indexed line number reported by httpyac.
    #[serde(default)]
    pub line: Option<u32>,
    pub response: Option<Response>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Response {
    #[serde(rename = "statusCode")]
    pub status_code: u16,
    #[serde(rename = "statusMessage", default)]
    pub status_message: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub headers: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub timings: Option<Timings>,
    #[serde(default)]
    pub meta: Option<Meta>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Timings {
    #[serde(default)]
    pub total: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub size: Option<String>,
}
