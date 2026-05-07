use std::time::Instant;

use reqwest::Client;

/// Flags controlling what extra data probe_url should extract.
pub struct ProbeConfig {
    pub fetch_title: bool,
    pub fetch_content_length: bool,
}

/// A successful probe response.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub url: String,
    pub status: u16,
    pub elapsed_ms: u64,
    pub content_length: Option<u64>,
    pub title: Option<String>,
}

/// Strip any existing scheme from user-supplied input so we always work
/// with a bare hostname (e.g. "dell.com" not "https://dell.com").
pub fn normalize_domain(input: &str) -> String {
    let s = input.trim();
    if let Some(rest) = s.strip_prefix("https://") {
        rest.to_string()
    } else if let Some(rest) = s.strip_prefix("http://") {
        rest.to_string()
    } else {
        s.to_string()
    }
}

/// Build the full list of URLs to probe for a single domain.
///
/// Port-to-scheme mapping (when --only is not set):
///   80   → http only   (standard HTTP port)
///   443  → https only  (standard HTTPS port)
///   else → both http and https on that port (unknown server type)
///
/// Standard ports are omitted from the URL (RFC 7230).
pub fn build_urls(domain: &str, ports: &[u16], only: Option<&str>) -> Vec<String> {
    let mut urls = Vec::with_capacity(ports.len() * 2);

    for &port in ports {
        // Skip the standard port for the opposite scheme — forcing HTTPS on port 80
        // (or HTTP on 443) causes the server to hang the handshake until timeout.
        if only == Some("https") && port == 80  { continue; }
        if only == Some("http")  && port == 443 { continue; }

        let schemes: &[&str] = match only {
            Some("https") => &["https"],
            Some("http") => &["http"],
            _ => match port {
                80 => &["http"],
                443 => &["https"],
                _ => &["http", "https"],
            },
        };

        for &scheme in schemes {
            let is_standard =
                (scheme == "http" && port == 80) || (scheme == "https" && port == 443);
            let url = if is_standard {
                format!("{}://{}", scheme, domain)
            } else {
                format!("{}://{}:{}", scheme, domain, port)
            };
            urls.push(url);
        }
    }

    urls
}

/// Extract `<title>...</title>` from raw HTML via simple string search.
fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let tag_end = lower[start..].find('>')? + start + 1;
    let end = lower[tag_end..].find("</title>")? + tag_end;
    let t = html[tag_end..end].trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}

/// Read the response body (decompressed) and return (title, total_bytes_read).
/// Uses .text() to get decompressed content matching httpx behavior.
async fn read_title(resp: reqwest::Response) -> (Option<String>, usize) {
    let body = resp.text().await.unwrap_or_default();
    let total_bytes = body.len();
    let title = extract_title(&body);
    (title, total_bytes)
}

/// Fire a single HTTP request and return a `ProbeResult` on success.
async fn probe_url(client: &Client, url: String, cfg: &ProbeConfig) -> Option<ProbeResult> {
    let start = Instant::now();
    let resp = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, fake_user_agent::get_rua())
        .send()
        .await
        .ok()?;

    let status = resp.status().as_u16();

    // Content-Length lives in headers — already downloaded, zero extra cost.
    let content_length = if cfg.fetch_content_length {
        resp.headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
    } else {
        None
    };

    // Title requires reading the body — only when the flag is set.
    // We also get the body size for Content-Length when header is missing.
    let (title, body_size) = if cfg.fetch_title {
        read_title(resp).await
    } else {
        (None, 0)
    };

    // Use header if available, otherwise use actual bytes read from body
    let content_length = if cfg.fetch_content_length {
        content_length.or_else(|| if body_size > 0 { Some(body_size as u64) } else { None })
    } else {
        None
    };

    Some(ProbeResult {
        url,
        status,
        elapsed_ms: start.elapsed().as_millis() as u64,
        content_length,
        title,
    })
}

/// Probe all URLs for a domain concurrently and return every successful result.
///
/// All requests are fired simultaneously via `futures::future::join_all`,
/// so the total wall-clock time is bounded by the slowest single response
/// (or the timeout), never the sum of all timeouts.
pub async fn probe_domain(
    client: &Client,
    domain: &str,
    ports: &[u16],
    only: Option<&str>,
    cfg: &ProbeConfig,
) -> Vec<ProbeResult> {
    let domain = normalize_domain(domain);
    let urls = build_urls(&domain, ports, only);

    let futs: Vec<_> = urls
        .into_iter()
        .map(|url| probe_url(client, url, cfg))
        .collect();

    futures::future::join_all(futs)
        .await
        .into_iter()
        .flatten()
        .collect()
}
