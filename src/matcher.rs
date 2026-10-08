use anyhow::{Context, Result};
use clap::ValueEnum;
use regex::Regex;

use crate::cli::Args;

/// Comparison operator for `--match-response-time`.
#[derive(Debug, Clone, Copy)]
enum Op {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
}

/// A parsed `--match-response-time` expression such as `< 1` (seconds).
#[derive(Debug, Clone, Copy)]
pub struct TimeCond {
    op: Op,
    secs: f64,
}

/// clap value parser for `--match-response-time`. Accepts `<`, `<=`, `>`, `>=`,
/// `=`/`==` followed by a number of seconds, with or without a space: `'< 1'`, `>=0.5`.
pub fn parse_time_cond(s: &str) -> Result<TimeCond, String> {
    let s = s.trim();
    let (op, rest) = if let Some(r) = s.strip_prefix("<=") {
        (Op::Le, r)
    } else if let Some(r) = s.strip_prefix(">=") {
        (Op::Ge, r)
    } else if let Some(r) = s.strip_prefix("==") {
        (Op::Eq, r)
    } else if let Some(r) = s.strip_prefix('<') {
        (Op::Lt, r)
    } else if let Some(r) = s.strip_prefix('>') {
        (Op::Gt, r)
    } else if let Some(r) = s.strip_prefix('=') {
        (Op::Eq, r)
    } else {
        return Err("expected an operator like '< 1' or '>= 0.5' (seconds)".into());
    };
    let secs = rest
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("'{}' is not a number of seconds", rest.trim()))?;
    Ok(TimeCond { op, secs })
}

impl TimeCond {
    fn test(&self, elapsed_ms: u64) -> bool {
        let t = elapsed_ms as f64 / 1000.0;
        match self.op {
            Op::Lt => t < self.secs,
            Op::Le => t <= self.secs,
            Op::Gt => t > self.secs,
            Op::Ge => t >= self.secs,
            Op::Eq => (t - self.secs).abs() < 0.0005,
        }
    }
}

/// Page types for `--filter-page-type`, detected with simple keyword heuristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PageType {
    Login,
    Captcha,
    Parked,
}

/// Guess which page types a response body looks like.
fn classify(body: &str) -> Vec<PageType> {
    let b = body.to_lowercase();
    let mut types = Vec::new();
    if b.contains("type=\"password\"") || b.contains("type='password'") || b.contains("type=password") {
        types.push(PageType::Login);
    }
    if ["captcha", "cf-turnstile"].iter().any(|k| b.contains(k)) {
        types.push(PageType::Captcha);
    }
    const PARKED: &[&str] = &[
        "domain is for sale",
        "domain may be for sale",
        "buy this domain",
        "this domain is parked",
        "domain parking",
        "parkingcrew",
        "sedoparking",
        "hugedomains",
        "afternic",
    ];
    if PARKED.iter().any(|k| b.contains(k)) {
        types.push(PageType::Parked);
    }
    types
}

/// What the matchers get to look at for one response.
pub struct Facts<'a> {
    pub status: u16,
    pub length: Option<u64>,
    pub lines: Option<usize>,
    pub words: Option<usize>,
    pub body: &'a str,
    pub elapsed_ms: u64,
}

/// A set of conditions used for both `--match-*` (keep) and `--filter-*` (drop).
/// Values inside one flag are OR-ed, and so are the different flags:
/// the set "hits" if ANY active condition hits.
pub struct Matchers {
    code: Vec<u16>,
    length: Vec<u64>,
    lines: Vec<usize>,
    words: Vec<usize>,
    favicon: Vec<i32>,
    strings: Vec<String>,
    regexes: Vec<Regex>,
    time: Option<TimeCond>,
    page_types: Vec<PageType>,
}

impl Matchers {
    pub fn from_matchers(a: &Args) -> Result<Self> {
        Self::build(
            "--match-regex",
            (&a.match_code, &a.match_length, &a.match_line_count, &a.match_word_count),
            &a.match_favicon,
            &a.match_string,
            &a.match_regex,
            a.match_response_time,
            Vec::new(),
        )
    }

    pub fn from_filters(a: &Args) -> Result<Self> {
        Self::build(
            "--filter-regex",
            (&a.filter_code, &a.filter_length, &a.filter_line_count, &a.filter_word_count),
            &a.filter_favicon,
            &a.filter_string,
            &a.filter_regex,
            a.filter_response_time,
            a.filter_page_type.clone(),
        )
    }

    fn build(
        flag: &str,
        nums: (&[u16], &[u64], &[usize], &[usize]),
        favicon: &[i32],
        strings: &[String],
        regexes: &[String],
        time: Option<TimeCond>,
        page_types: Vec<PageType>,
    ) -> Result<Self> {
        let regexes = regexes
            .iter()
            .map(|r| Regex::new(r).with_context(|| format!("invalid {} '{}'", flag, r)))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            code: nums.0.to_vec(),
            length: nums.1.to_vec(),
            lines: nums.2.to_vec(),
            words: nums.3.to_vec(),
            favicon: favicon.to_vec(),
            strings: strings.to_vec(),
            regexes,
            time,
            page_types,
        })
    }

    /// True if at least one `--match-*` flag was given.
    pub fn is_active(&self) -> bool {
        !(self.code.is_empty()
            && self.length.is_empty()
            && self.lines.is_empty()
            && self.words.is_empty()
            && self.favicon.is_empty()
            && self.strings.is_empty()
            && self.regexes.is_empty()
            && self.time.is_none()
            && self.page_types.is_empty())
    }

    pub fn has_length(&self) -> bool {
        !self.length.is_empty()
    }

    pub fn has_favicon(&self) -> bool {
        !self.favicon.is_empty()
    }

    /// Matchers that look inside the body need it downloaded.
    pub fn needs_body(&self) -> bool {
        !(self.lines.is_empty() && self.words.is_empty() && self.strings.is_empty())
            || !self.regexes.is_empty()
            || !self.page_types.is_empty()
    }

    /// Every matcher except favicon (which costs an extra request, so it runs last).
    pub fn matches_basic(&self, f: &Facts) -> bool {
        self.code.contains(&f.status)
            || f.length.is_some_and(|l| self.length.contains(&l))
            || f.lines.is_some_and(|l| self.lines.contains(&l))
            || f.words.is_some_and(|w| self.words.contains(&w))
            || self.strings.iter().any(|s| f.body.contains(s.as_str()))
            || self.regexes.iter().any(|r| r.is_match(f.body))
            || self.time.is_some_and(|t| t.test(f.elapsed_ms))
            || (!self.page_types.is_empty() && {
                let found = classify(f.body);
                self.page_types.iter().any(|p| found.contains(p))
            })
    }

    pub fn matches_favicon(&self, hash: Option<i32>) -> bool {
        hash.is_some_and(|h| self.favicon.contains(&h))
    }
}
