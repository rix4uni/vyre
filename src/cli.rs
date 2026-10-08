use std::time::Duration;

use clap::Parser;

/// Parse durations like `200ms`, `1s`, `1.5s`, `2m`, `1h`. A bare number means seconds.
fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let (num, mult) = if let Some(n) = s.strip_suffix("ms") {
        (n, 0.001)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1.0)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60.0)
    } else if let Some(n) = s.strip_suffix('h') {
        (n, 3600.0)
    } else {
        (s, 1.0)
    };
    let v: f64 = num
        .trim()
        .parse()
        .map_err(|_| format!("'{}' is not a duration (try 200ms, 1s, 2m)", s))?;
    if v < 0.0 {
        return Err("duration cannot be negative".into());
    }
    Ok(Duration::from_secs_f64(v * mult))
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "vyre",
    about = "High-performance subdomain prober — fires HTTP & HTTPS simultaneously",
    version,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub struct Args {
    /// Print help
    #[arg(long = "help", action = clap::ArgAction::Help)]
    help: Option<bool>,

    /// Print version
    #[arg(long = "version", action = clap::ArgAction::Version)]
    version: Option<bool>,

    /// Max concurrent probes
    #[arg(help_heading = "SCAN OPTIONS", long = "concurrency", default_value_t = 1000)]
    pub concurrency: usize,

    /// Request timeout in seconds
    #[arg(help_heading = "SCAN OPTIONS", long = "timeout", default_value_t = 10)]
    pub timeout: u64,

    /// Comma-separated ports to probe (e.g. 80,443,8080,8443)
    #[arg(help_heading = "SCAN OPTIONS", long = "ports", default_value = "80,443")]
    pub ports: String,

    /// Restrict to a single protocol: http or https
    #[arg(help_heading = "SCAN OPTIONS", long = "only", value_parser = ["http", "https"])]
    pub only: Option<String>,

    /// Write live results to a file (also prints to stdout)
    #[arg(help_heading = "SCAN OPTIONS", long = "output")]
    pub output: Option<String>,

    /// Suppress banner; errors still go to stderr
    #[arg(help_heading = "SCAN OPTIONS", long = "silent")]
    pub silent: bool,

    /// Extract and print the page <title> tag (reads first 8 KB of body)
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "title")]
    pub title: bool,

    /// Print the Content-Length response header value
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "content-length")]
    pub content_length: bool,

    /// Show response Content-Type header
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "content-type")]
    pub content_type: bool,

    /// Show redirect Location header
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "location")]
    pub location: bool,

    /// Show mmh3 hash of /favicon.ico (extra request per URL)
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "favicon")]
    pub favicon: bool,

    /// Show response body line count (reads the full body)
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "line-count")]
    pub line_count: bool,

    /// Show response body word count (reads the full body)
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "word-count")]
    pub word_count: bool,

    /// Show Server response header
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "server")]
    pub server: bool,

    /// Show the IP address of the host that responded
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "ip")]
    pub ip: bool,

    /// Show the host CNAME record (extra DNS lookup per domain)
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "cname")]
    pub cname: bool,

    /// Match status codes, e.g. 200,302
    #[arg(help_heading = "MATCHERS (keep a result if ANY given matcher matches)", long = "match-code", value_delimiter = ',')]
    pub match_code: Vec<u16>,

    /// Match Content-Length values, e.g. 100,102
    #[arg(help_heading = "MATCHERS (keep a result if ANY given matcher matches)", long = "match-length", value_delimiter = ',')]
    pub match_length: Vec<u64>,

    /// Match body line counts, e.g. 423,532
    #[arg(help_heading = "MATCHERS (keep a result if ANY given matcher matches)", long = "match-line-count", value_delimiter = ',')]
    pub match_line_count: Vec<usize>,

    /// Match body word counts, e.g. 43,55
    #[arg(help_heading = "MATCHERS (keep a result if ANY given matcher matches)", long = "match-word-count", value_delimiter = ',')]
    pub match_word_count: Vec<usize>,

    /// Match favicon hashes, e.g. 1494302000 (see --favicon)
    #[arg(help_heading = "MATCHERS (keep a result if ANY given matcher matches)", long = "match-favicon", value_delimiter = ',', allow_negative_numbers = true)]
    pub match_favicon: Vec<i32>,

    /// Match body text, e.g. admin (comma-separated or repeat the flag)
    #[arg(help_heading = "MATCHERS (keep a result if ANY given matcher matches)", long = "match-string", value_delimiter = ',')]
    pub match_string: Vec<String>,

    /// Match body against a regex, e.g. 'admin|login' (repeat the flag for more)
    #[arg(help_heading = "MATCHERS (keep a result if ANY given matcher matches)", long = "match-regex")]
    pub match_regex: Vec<String>,

    /// Match response time in seconds, e.g. '< 1' or '>= 0.5'
    #[arg(help_heading = "MATCHERS (keep a result if ANY given matcher matches)", long = "match-response-time", value_parser = crate::matcher::parse_time_cond, allow_hyphen_values = true)]
    pub match_response_time: Option<crate::matcher::TimeCond>,

    /// Drop status codes, e.g. 403,401
    #[arg(help_heading = "FILTERS (drop a result if ANY given filter matches)", long = "filter-code", value_delimiter = ',')]
    pub filter_code: Vec<u16>,

    /// Drop page types (login, captcha, parked), detected by keyword heuristics
    #[arg(help_heading = "FILTERS (drop a result if ANY given filter matches)", long = "filter-page-type", value_delimiter = ',', value_enum)]
    pub filter_page_type: Vec<crate::matcher::PageType>,

    /// Drop Content-Length values, e.g. 23,33
    #[arg(help_heading = "FILTERS (drop a result if ANY given filter matches)", long = "filter-length", value_delimiter = ',')]
    pub filter_length: Vec<u64>,

    /// Drop body line counts, e.g. 423,532
    #[arg(help_heading = "FILTERS (drop a result if ANY given filter matches)", long = "filter-line-count", value_delimiter = ',')]
    pub filter_line_count: Vec<usize>,

    /// Drop body word counts, e.g. 423,532
    #[arg(help_heading = "FILTERS (drop a result if ANY given filter matches)", long = "filter-word-count", value_delimiter = ',')]
    pub filter_word_count: Vec<usize>,

    /// Drop favicon hashes, e.g. 1494302000 (see --favicon)
    #[arg(help_heading = "FILTERS (drop a result if ANY given filter matches)", long = "filter-favicon", value_delimiter = ',', allow_negative_numbers = true)]
    pub filter_favicon: Vec<i32>,

    /// Drop body text, e.g. admin (comma-separated or repeat the flag)
    #[arg(help_heading = "FILTERS (drop a result if ANY given filter matches)", long = "filter-string", value_delimiter = ',')]
    pub filter_string: Vec<String>,

    /// Drop bodies matching a regex, e.g. 'admin|login' (repeat the flag for more)
    #[arg(help_heading = "FILTERS (drop a result if ANY given filter matches)", long = "filter-regex")]
    pub filter_regex: Vec<String>,

    /// Drop by response time in seconds, e.g. '> 1'
    #[arg(help_heading = "FILTERS (drop a result if ANY given filter matches)", long = "filter-response-time", value_parser = crate::matcher::parse_time_cond, allow_hyphen_values = true)]
    pub filter_response_time: Option<crate::matcher::TimeCond>,

    /// Use a random User-Agent for each request (default: a fixed Chrome one)
    #[arg(help_heading = "REQUESTS", long = "random-agent")]
    pub random_agent: bool,

    /// Follow HTTP redirects
    #[arg(help_heading = "REQUESTS", long = "follow-redirects")]
    pub follow_redirects: bool,

    /// Max number of redirects to follow per host
    #[arg(help_heading = "REQUESTS", long = "max-redirects", default_value_t = 10)]
    pub max_redirects: usize,

    /// Follow redirects that stay on the same host
    #[arg(help_heading = "REQUESTS", long = "follow-host-redirects")]
    pub follow_host_redirects: bool,

    /// Number of retries for failed requests
    #[arg(help_heading = "REQUESTS", long = "retries", default_value_t = 0)]
    pub retries: u32,

    /// Minimum time between requests (e.g. 200ms, 1s)
    #[arg(help_heading = "REQUESTS", long = "delay", value_parser = parse_duration)]
    pub delay: Option<Duration>,

    /// Disable colors in output
    #[arg(help_heading = "REQUESTS", long = "no-color")]
    pub no_color: bool,

    /// Show HTTP status code in output
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "status-code")]
    pub show_status: bool,

    /// Show response time in output
    #[arg(help_heading = "OUTPUT FIELDS (each one adds a column to the result line)", long = "response-time")]
    pub show_rt: bool,

    /// Print scan statistics (input / alive / dead) to stderr after the scan
    #[arg(help_heading = "SCAN OPTIONS", long = "stats")]
    pub stats: bool,
}

/// Single-dash multi-letter shorthands. clap only supports one-character short
/// flags, so these are rewritten to their long form before parsing.
const SHORTHANDS: &[(&str, &str)] = &[
    ("-cl", "--content-length"),
    ("-ct", "--content-type"),
    ("-sc", "--status-code"),
    ("-rt", "--response-time"),
    ("-lc", "--line-count"),
    ("-wc", "--word-count"),
    ("-mc", "--match-code"),
    ("-ml", "--match-length"),
    ("-mlc", "--match-line-count"),
    ("-mwc", "--match-word-count"),
    ("-mfc", "--match-favicon"),
    ("-ms", "--match-string"),
    ("-mr", "--match-regex"),
    ("-mrt", "--match-response-time"),
    ("-fc", "--filter-code"),
    ("-fpt", "--filter-page-type"),
    ("-fl", "--filter-length"),
    ("-flc", "--filter-line-count"),
    ("-fwc", "--filter-word-count"),
    ("-ffc", "--filter-favicon"),
    ("-fs", "--filter-string"),
    ("-fe", "--filter-regex"),
    ("-frt", "--filter-response-time"),
    ("-fr", "--follow-redirects"),
    ("-maxr", "--max-redirects"),
    ("-fhr", "--follow-host-redirects"),
    ("-nc", "--no-color"),
];

const HELP: &str = r#"High-performance subdomain prober written in Rust. Like httpx, but faster.

Usage:
  echo example.com | vyre [flags]
  cat subs.txt | vyre [flags]

Flags:
PROBES:
   -sc, --status-code      display response status-code
   -cl, --content-length   display response content-length
   -ct, --content-type     display response content-type
   --location              display response redirect location
   --favicon               display mmh3 hash for '/favicon.ico' file
   -rt, --response-time    display response time
   -lc, --line-count       display response body line count
   -wc, --word-count       display response body word count
   --title                 display page title
   --server                display server name
   --ip                    display host ip
   --cname                 display host cname

MATCHERS:
   -mc, --match-code string             match response with specified status code (-mc 200,302)
   -ml, --match-length string           match response with specified content length (-ml 100,102)
   -mlc, --match-line-count string      match response body with specified line count (-mlc 423,532)
   -mwc, --match-word-count string      match response body with specified word count (-mwc 43,55)
   -mfc, --match-favicon string[]       match response with specified favicon hash (-mfc 1494302000)
   -ms, --match-string string[]         match response with specified string (-ms admin)
   -mr, --match-regex string[]          match response with specified regex (-mr admin)
   -mrt, --match-response-time string   match response with specified response time in seconds (-mrt '< 1')

FILTERS:
   -fc, --filter-code string             filter response with specified status code (-fc 403,401)
   -fpt, --filter-page-type string[]     filter response with specified page type (e.g. -fpt login,captcha,parked)
   -fl, --filter-length string           filter response with specified content length (-fl 23,33)
   -flc, --filter-line-count string      filter response body with specified line count (-flc 423,532)
   -fwc, --filter-word-count string      filter response body with specified word count (-fwc 423,532)
   -ffc, --filter-favicon string[]       filter response with specified favicon hash (-ffc 1494302000)
   -fs, --filter-string string[]         filter response with specified string (-fs admin)
   -fe, --filter-regex string[]          filter response with specified regex (-fe admin)
   -frt, --filter-response-time string   filter response with specified response time in seconds (-frt '> 1')

RATE-LIMIT:
   --concurrency int   number of concurrent probes (default 1000)
   --delay value       duration between each http request (eg: 200ms, 1s)

MISCELLANEOUS:
   --ports string   ports to probe, comma separated (default 80,443)
   --only string    probe only one protocol: http or https

OUTPUT:
   --output string   file to write output results

CONFIGURATIONS:
   --random-agent                  enable Random User-Agent to use (default false)
   -fr, --follow-redirects         follow http redirects
   -maxr, --max-redirects int      max number of redirects to follow per host (default 10)
   -fhr, --follow-host-redirects   follow redirects on the same host

DEBUG:
   --help            display help
   --version         display vyre version
   --stats           display scan statistic
   --silent          silent mode
   -nc, --no-color   disable colors in cli output

OPTIMIZATIONS:
   --retries int   number of retries
   --timeout int   timeout in seconds (default 10)

"#;

/// Parse CLI args, expanding the shorthands above.
pub fn parse() -> Args {
    let raw: Vec<String> = std::env::args().collect();
    if raw.iter().skip(1).any(|a| a == "--help") {
        print!("{}", HELP);
        std::process::exit(0);
    }
    let argv = raw.into_iter().enumerate().map(|(i, a)| {
        if i == 0 {
            return a;
        }
        match SHORTHANDS.iter().find(|(s, _)| *s == a) {
            Some((_, long)) => long.to_string(),
            None => a,
        }
    });
    Args::parse_from(argv)
}

impl Args {
    /// Parse the --ports string into a deduplicated, sorted list of u16 port numbers.
    pub fn parsed_ports(&self) -> Vec<u16> {
        let mut ports: Vec<u16> = self
            .ports
            .split(',')
            .filter_map(|p| p.trim().parse::<u16>().ok())
            .collect();
        ports.sort_unstable();
        ports.dedup();
        ports
    }
}
