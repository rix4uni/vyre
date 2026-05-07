use crate::prober::ProbeResult;

const RESET: &str = "\x1b[0m";
const PURPLE: &str = "\x1b[95m";
const CYAN: &str = "\x1b[36m";
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
}

/// Format a probe result according to the active display flags.
///
/// Always shown : URL
/// --sc / --status-code  : [200]
/// --rt / --response-time: [120ms]
/// --content-length      : [cl:45231]   (when fetched and present)
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
    if let Some(ref title) = r.title {
        line.push_str(&format!(" {}[{}]{}", CYAN, title, RESET));
    }
    if cfg.show_rt {
        line.push_str(&format!(" {}[{}ms]{}", PURPLE, r.elapsed_ms, RESET));
    }
    line.push('\n');
    line
}

pub fn banner() -> String {
    format!(
        r#"{}
 _   __ __  __ _____ ___
| | / // / / // ___// _ \
| |/ // /_/ // /   /  __/
|___/ \__, //_/    \___/
     /____/              v{}

{}Use with caution. You are responsible for your actions.
Developers assume no liability and are not responsible for any misuse or damage.{}
"#,
        "\x1b[36m", env!("CARGO_PKG_VERSION"), "\x1b[90m", "\x1b[0m"
    )
}
