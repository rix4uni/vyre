use crate::prober::ProbeResult;

const RESET: &str = "\x1b[0m";
const PURPLE: &str = "\x1b[95m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "[93m";
const GREEN: &str = "[92m";
const BRIGHT_BLUE: &str = "\x1b[94m";

#[inline]
fn status_color(status: u16) -> &'static str {
    match status {
        200..=299 => "\x1b[32m",   // green
        300..=399 => "\x1b[33m",   // yellow
        400..=499 => "\x1b[31m",   // red
        500..=599 => "\x1b[91m",   // bright red
        _         => "",
    }
}

/// Controls which fields appear in the output line.
pub struct DisplayConfig {
    pub show_status: bool,
    pub show_rt: bool,
    pub color: bool,
}

/// Format a probe result according to the active display flags.
///
/// Always shown : URL
/// --status-code  : [200]
/// --response-time: [120ms]
/// --content-length      : [cl:45231]   (when fetched and present)
/// --favicon             : [-1234567]   (mmh3 of /favicon.ico, when found)
/// --line-count          : [42]         (body lines)
/// --location            : [https://x.com/] (when present)
/// --content-type        : [text/html]  (when present)
/// --title               : [Page Title] (when fetched and present)
#[inline]
pub fn format_result(r: &ProbeResult, cfg: &DisplayConfig) -> String {
    let mut line = r.url.clone();
    if cfg.show_status {
        let color = status_color(r.status);
        line.push_str(&format!(" {}[{}]{}", color, r.status, RESET));
    }
    if let Some(cl) = r.content_length {
        line.push_str(&format!(" {}[{}]{}", BRIGHT_BLUE, cl, RESET));
    }
    if let Some(fh) = r.favicon_hash {
        line.push_str(&format!(" {}[{}]{}", PURPLE, fh, RESET));
    }
    if let Some(ref loc) = r.location {
        line.push_str(&format!(" {}[{}]{}", GREEN, loc, RESET));
    }
    if let Some(lc) = r.line_count {
        line.push_str(&format!(" {}[{}]{}", BRIGHT_BLUE, lc, RESET));
    }
    if let Some(wc) = r.word_count {
        line.push_str(&format!(" {}[{}]{}", BRIGHT_BLUE, wc, RESET));
    }
    if let Some(ref srv) = r.server {
        line.push_str(&format!(" {}[{}]{}", CYAN, srv, RESET));
    }
    if let Some(ref ip) = r.ip {
        line.push_str(&format!(" {}[{}]{}", PURPLE, ip, RESET));
    }
    if let Some(ref cname) = r.cname {
        line.push_str(&format!(" {}[{}]{}", YELLOW, cname, RESET));
    }
    if let Some(ref ct) = r.content_type {
        line.push_str(&format!(" {}[{}]{}", YELLOW, ct, RESET));
    }
    if let Some(ref title) = r.title {
        line.push_str(&format!(" {}[{}]{}", CYAN, title, RESET));
    }
    if cfg.show_rt {
        line.push_str(&format!(" {}[{}ms]{}", PURPLE, r.elapsed_ms, RESET));
    }
    line.push('\n');
    if cfg.color { line } else { strip_ansi(&line) }
}

/// Remove ANSI colour escape sequences (`ESC [ ... m`) for --no-color.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for n in chars.by_ref() {
                if n == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn banner() -> String {
    format!(
        r#"{}
 _   __ __  __ _____ ___
| | / // / / // ___// _ \
| |/ // /_/ // /   /  __/
|___/ \__, //_/    \___/   {}v{}{}
     /____/            {}fast subdomain prober{}

"#,
        "\x1b[36m",
        "\x1b[1;97m", env!("CARGO_PKG_VERSION"), "\x1b[36m",
        "\x1b[90m", "\x1b[0m"
    )
}
