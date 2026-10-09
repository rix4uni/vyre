use std::time::{Duration, Instant};

use hickory_resolver::proto::rr::{RData, RecordType};
use hickory_resolver::TokioAsyncResolver;
use reqwest::Client;

use crate::matcher::{Facts, Matchers};

/// Flags controlling what extra data probe_url should extract.
pub struct ProbeConfig {
    pub fetch_title: bool,
    pub fetch_content_length: bool,
    pub fetch_content_type: bool,
    pub fetch_location: bool,
    pub fetch_favicon: bool,
    pub fetch_line_count: bool,
    pub fetch_word_count: bool,
    pub fetch_server: bool,
    pub fetch_ip: bool,
    /// Present only when --cname is set.
    pub resolver: Option<TokioAsyncResolver>,
    pub matchers: Matchers,
    pub filters: Matchers,
    pub retries: u32,
    /// Present only when --delay is set.
    pub pacer: Option<Pacer>,
}

/// Spaces requests at least `gap` apart across ALL concurrent probes.
pub struct Pacer {
    gap: Duration,
    next: tokio::sync::Mutex<Instant>,
}

impl Pacer {
    pub fn new(gap: Duration) -> Self {
        Self { gap, next: tokio::sync::Mutex::new(Instant::now()) }
    }

    /// Reserve the next free slot and sleep until it arrives.
    async fn wait(&self) {
        let at = {
            let mut next = self.next.lock().await;
            let slot = (*next).max(Instant::now());
            *next = slot + self.gap;
            slot
        };
        tokio::time::sleep_until(tokio::time::Instant::from_std(at)).await;
    }
}

/// A fresh random browser User-Agent is used for every request.
fn user_agent(_cfg: &ProbeConfig) -> String {
    fake_user_agent::get_rua().to_string()
}

/// A successful probe response.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub url: String,
    pub status: u16,
    pub elapsed_ms: u64,
    pub content_length: Option<u64>,
    pub title: Option<String>,
    pub content_type: Option<String>,
    pub location: Option<String>,
    pub favicon_hash: Option<i32>,
    pub line_count: Option<usize>,
    pub word_count: Option<usize>,
    pub server: Option<String>,
    pub ip: Option<String>,
    pub cname: Option<String>,
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

    // A port baked into the input host (e.g. "host:443") pins the real scheme:
    // 443 is HTTPS-only and 80 is HTTP-only, so probing the other scheme just
    // yields a bogus 400 ("plain HTTP request was sent to HTTPS port").
    let embedded_port = domain
        .rsplit_once(':')
        .and_then(|(_, p)| p.parse::<u16>().ok());

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
            // Skip HTTP against an embedded :443 host and HTTPS against :80.
            if embedded_port == Some(443) && scheme == "http" {
                continue;
            }
            if embedded_port == Some(80) && scheme == "https" {
                continue;
            }
            let is_standard =
                (scheme == "http" && port == 80) || (scheme == "https" && port == 443);
            let mut url = if is_standard {
                format!("{}://{}", scheme, domain)
            } else {
                format!("{}://{}:{}", scheme, domain, port)
            };
            // Drop a default web port the input host carried (e.g. "host:443"):
            // 443 and 80 are the HTTPS/HTTP defaults, so showing them is noise.
            if let Some(trimmed) = url
                .strip_suffix(":443")
                .or_else(|| url.strip_suffix(":80"))
            {
                url = trimmed.to_string();
            }
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

/// Fire a single HTTP request and return a `ProbeResult` on success.
async fn probe_url(client: &Client, url: String, cfg: &ProbeConfig) -> Option<ProbeResult> {
    // Retry failed requests (connection errors, timeouts) up to `retries` times.
    let mut tries = 0;
    let (start, resp) = loop {
        if let Some(p) = &cfg.pacer {
            p.wait().await;
        }
        let start = Instant::now();
        match client
            .get(&url)
            .header(reqwest::header::USER_AGENT, user_agent(cfg))
            .send()
            .await
        {
            Ok(r) => break (start, r),
            Err(_) if tries < cfg.retries => tries += 1,
            Err(_) => return None,
        }
    };

    let status = resp.status().as_u16();

    // Content-Length lives in headers - already downloaded, zero extra cost.
    let header_len = resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    // Content-Type also lives in headers; keep only the media type, not parameters.
    let content_type = if cfg.fetch_content_type {
        resp.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or(s).trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    // Redirect target (redirects are never followed, so this is the raw header).
    let location = if cfg.fetch_location {
        resp.headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    // Server banner lives in headers — zero extra cost.
    let server = if cfg.fetch_server {
        resp.headers()
            .get(reqwest::header::SERVER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    // Peer address of the connection that actually answered — no extra DNS lookup.
    let ip = if cfg.fetch_ip {
        resp.remote_addr().map(|a| a.ip().to_string())
    } else {
        None
    };

    let m = &cfg.matchers;
    let f = &cfg.filters;

    // The body is only downloaded when something needs it.
    let need_body = cfg.fetch_title
        || cfg.fetch_line_count
        || cfg.fetch_word_count
        || m.needs_body()
        || f.needs_body()
        || ((m.has_length() || f.has_length()) && header_len.is_none());
    let body = if need_body {
        resp.text().await.unwrap_or_default()
    } else {
        String::new()
    };

    // Use the header if available, otherwise the actual bytes read from the body.
    let length = header_len.or_else(|| {
        if need_body && !body.is_empty() { Some(body.len() as u64) } else { None }
    });
    let title = if cfg.fetch_title { extract_title(&body) } else { None };
    // httpx counts newline characters + 1; an empty body has 0 lines.
    let lines = if need_body {
        Some(if body.is_empty() { 0 } else { body.matches('\n').count() + 1 })
    } else {
        None
    };
    let words = if need_body { Some(body.split_whitespace().count()) } else { None };

    let elapsed_ms = start.elapsed().as_millis() as u64;

    let facts = Facts {
        status,
        length,
        lines,
        words,
        body: &body,
        elapsed_ms,
    };

    // --filter-*: any hit drops the result. --match-*: any hit keeps it.
    // Cheap checks first; the favicon (extra request) only if still undecided.
    if f.matches_basic(&facts) {
        return None;
    }
    let mut matched = !m.is_active() || m.matches_basic(&facts);
    let favicon_hash = if cfg.fetch_favicon
        || (!matched && m.has_favicon())
        || f.has_favicon()
    {
        fetch_favicon_hash(client, &url, cfg).await
    } else {
        None
    };
    if f.matches_favicon(favicon_hash) {
        return None;
    }
    if !matched && m.has_favicon() {
        matched = m.matches_favicon(favicon_hash);
    }
    if !matched {
        return None;
    }

    let content_length = if cfg.fetch_content_length { length } else { None };
    let line_count = if cfg.fetch_line_count { lines } else { None };
    let word_count = if cfg.fetch_word_count { words } else { None };
    let favicon_hash = if cfg.fetch_favicon { favicon_hash } else { None };

    Some(ProbeResult {
        url,
        status,
        elapsed_ms,
        content_length,
        title,
        content_type,
        location,
        favicon_hash,
        line_count,
        word_count,
        server,
        ip,
        cname: None,
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

    // CNAME is per-domain, so resolve it once, concurrently with the probes.
    let (results, cname) = futures::join!(
        futures::future::join_all(futs),
        lookup_cname(cfg.resolver.as_ref(), &domain)
    );

    results
        .into_iter()
        .flatten()
        .map(|mut r| {
            r.cname = cname.clone();
            r
        })
        .collect()
}

/// MIME-style base64 (76-char lines, each terminated by '\n'), matching
/// Python's `base64.encodebytes` — the form Shodan/httpx hash favicons over.
fn base64_mime(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len() * 4 / 3 + data.len() / 57 + 4);
    for chunk in data.chunks(57) {
        for tri in chunk.chunks(3) {
            let b = [tri[0], *tri.get(1).unwrap_or(&0), *tri.get(2).unwrap_or(&0)];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            out.push(T[(n >> 18) as usize & 63] as char);
            out.push(T[(n >> 12) as usize & 63] as char);
            out.push(if tri.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
            out.push(if tri.len() > 2 { T[n as usize & 63] as char } else { '=' });
        }
        out.push('\n');
    }
    out
}

/// MurmurHash3 x86 32-bit, seed 0, returned as signed i32 (like Python mmh3.hash).
fn murmur3_32(data: &[u8]) -> i32 {
    const C1: u32 = 0xcc9e2d51;
    const C2: u32 = 0x1b873593;
    let mut h: u32 = 0;
    let mut chunks = data.chunks_exact(4);
    for c in &mut chunks {
        let mut k = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        h ^= k;
        h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe6546b64);
    }
    let rem = chunks.remainder();
    if !rem.is_empty() {
        let mut k: u32 = 0;
        for (i, &b) in rem.iter().enumerate() {
            k |= (b as u32) << (8 * i);
        }
        k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        h ^= k;
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85ebca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    h as i32
}

/// Fetch `/favicon.ico` from the probed origin and return its mmh3 hash.
async fn fetch_favicon_hash(client: &Client, base_url: &str, cfg: &ProbeConfig) -> Option<i32> {
    if let Some(p) = &cfg.pacer {
        p.wait().await;
    }
    let resp = client
        .get(format!("{}/favicon.ico", base_url))
        .header(reqwest::header::USER_AGENT, user_agent(cfg))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.bytes().await.ok()?;
    if body.is_empty() {
        return None;
    }
    Some(murmur3_32(base64_mime(&body).as_bytes()))
}

/// Resolve the final CNAME target of `domain`, if it has one.
async fn lookup_cname(resolver: Option<&TokioAsyncResolver>, domain: &str) -> Option<String> {
    let lookup = resolver?.lookup(domain, RecordType::CNAME).await.ok()?;
    lookup
        .record_iter()
        .filter_map(|r| match r.data() {
            Some(RData::CNAME(c)) => Some(c.0.to_utf8().trim_end_matches('.').to_string()),
            _ => None,
        })
        .last()
}
