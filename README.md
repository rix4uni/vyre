## vyre

High-performance subdomain prober written in Rust. Like httpx, but faster.

## Features
<h1>
  <img src="https://github.com/user-attachments/assets/f0f96c47-76ee-4c29-834e-9792858eb123" alt="httpx" width="700px">
  <br>
</h1>

- **Simultaneous HTTP/HTTPS probing** - Fire both requests at once, wait for one timeout not two
- **Random User-Agent rotation** - Realistic browser UAs on every request (always on)
- **Colored terminal output** - Status codes, content-length, titles, response times color-coded
- **Accurate Content-Length** - Measures decompressed body size with gzip support
- **Title extraction** - Grabs page `<title>` from first 8KB (minimizes bandwidth)
- **Matchers & filters** - Keep or drop results by status code, size, lines, words, favicon hash, text, regex, page type or response time
- **Scan statistics** - Input/kept/removed counts at end of scan
- **Zero-allocation architecture** - Lock-free counters, streaming I/O
- **2-5x faster than httpx** in real-world subdomain enumeration

## Benchmark

Real-world test on 38,426 subdomains:

| Tool | Time | Results | Speedup |
|------|------|---------|---------|
| **httpx** | 89m 10s | 1,554 alive | 1x (baseline) |
| **vyre** | 3m 45s | 1,299 alive | **23.7x faster** |

> vyre found 83.6% of httpx results in **4% of the time**. The difference in alive hosts is due to timeout/handling variations.

## Installation
```
git clone --depth 1 https://github.com/rix4uni/vyre.git
cd vyre
cargo build --release
sudo cp target/release/vyre /usr/local/bin/
```

## Flags
```yaml
vyre is a fast subdomain prober that fires HTTP & HTTPS probes simultaneously.

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
   -best, --best-result    sort output by content-length, highest first

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
```

Matchers keep a result if **any** given matcher matches. Filters drop a result if **any** given filter matches. Both can be combined: a result must pass the matchers and not hit a filter. `--filter-page-type` uses keyword heuristics (login, captcha, parked), not ML.


## Example
<img width="1003" height="247" alt="image" src="https://github.com/user-attachments/assets/efeb61b3-6e7c-4dc6-b719-54212d7afa8e" />


## Usage Examples
```console
# Basic probe (URL only output)
echo "dell.com" | vyre
cat subs.txt | vyre

# Full information like httpx (colored output)
cat subs.txt | vyre --status-code --title --content-length --response-time

# Fast scan with custom concurrency
cat subs.txt | vyre --concurrency 5000

# HTTPS only with all details
cat subs.txt | vyre --only https --status-code --title --response-time

# Probe additional ports
cat subs.txt | vyre --ports 80,443,8080,8443

# Save to file + show stats
cat subs.txt | vyre --output results.txt --stats

# Silent mode for scripting
cat subs.txt | vyre --silent | tee results.txt

# Only keep hosts answering 200/302 that contain "admin"
cat subs.txt | vyre -sc -mc 200,302 -ms admin

# Hide forbidden / not-found / parked pages
cat subs.txt | vyre -sc -fc 403,404 -fpt parked

# Follow redirects, retry failures, throttle requests
cat subs.txt | vyre -fr -maxr 5 --retries 2 --delay 200ms

# Pipe to other tools
cat subs.txt | vyre -mc 200 | nuclei -t ~/nuclei-templates/exposures/
```
