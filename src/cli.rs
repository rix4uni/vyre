use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "vyre",
    about = "High-performance subdomain prober — fires HTTP & HTTPS simultaneously",
    version
)]
pub struct Args {
    /// Max concurrent probes
    #[arg(short = 'c', long = "concurrency", default_value_t = 1000)]
    pub concurrency: usize,

    /// Request timeout in seconds
    #[arg(short = 't', long = "timeout", default_value_t = 10)]
    pub timeout: u64,

    /// Comma-separated ports to probe (e.g. 80,443,8080,8443)
    #[arg(short = 'p',long = "ports", default_value = "80,443")]
    pub ports: String,

    /// Restrict to a single protocol: http or https
    #[arg(long = "only", value_parser = ["http", "https"])]
    pub only: Option<String>,

    /// Write live results to a file (also prints to stdout)
    #[arg(short = 'o', long = "output")]
    pub output: Option<String>,

    /// Suppress banner; errors still go to stderr
    #[arg(long = "silent")]
    pub silent: bool,

    /// Extract and print the page <title> tag (reads first 8 KB of body)
    #[arg(long = "title")]
    pub title: bool,

    /// Print the Content-Length response header value
    #[arg(long = "content-length", visible_alias = "ct")]
    pub content_length: bool,

    /// Show HTTP status code in output
    #[arg(long = "status-code", visible_alias = "sc")]
    pub show_status: bool,

    /// Show response time in output
    #[arg(long = "response-time", visible_alias = "rt")]
    pub show_rt: bool,

    /// Print scan statistics (input / alive / dead) to stderr after the scan
    #[arg(long = "stats")]
    pub stats: bool,
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
