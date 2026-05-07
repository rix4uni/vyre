## vyre

High-performance subdomain prober written in Rust. Like httpx, but faster.

## Features
- **Simultaneous HTTP/HTTPS probing** - Fire both requests at once, wait for one timeout not two
- **Random User-Agent rotation** - Realistic browser UAs on every request (no flag needed)
- **Colored terminal output** - Status codes, content-length, titles, response times color-coded
- **Accurate Content-Length** - Measures decompressed body size with gzip support
- **Title extraction** - Grabs page `<title>` from first 8KB (minimizes bandwidth)
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
# Binary: target/release/vyre
sudo cp target/release/vyre /usr/local/bin/
```

## Flags
| Flag | Shorthand | Default | Description |
|------|-----------|---------|-------------|
| `--concurrency` | `-c` | 1000 | Max simultaneous HTTP requests |
| `--timeout` | `-t` | 10 | Per-request timeout in seconds |
| `--ports` | `-p` | 80,443 | Comma-separated ports to probe |
| `--only` | - | - | Restrict to `http` or `https` |
| `--output` | `-o` | - | Write live results to file |
| `--silent` | - | false | Suppress banner output |
| `--status-code` | `--sc` | false | Show HTTP status code |
| `--response-time` | `--rt` | false | Show response time in milliseconds |
| `--title` | - | false | Extract page `<title>` tag (reads first 8KB) |
| `--content-length` | `--ct` | false | Show Content-Length (decompressed) |
| `--stats` | - | false | Print scan statistics (input/kept/removed) |


## Example
```console
$ echo "krazeplanet.com" | vyre --status-code --title --content-length --response-time

 _   __ __  __ _____ ___
| | / // / / // ___// _ \
| |/ // /_/ // /   /  __/
|___/ \__, //_/    \___/      v0.1.0
     /____/

Use with caution. You are responsible for your actions.
Developers assume no liability and are not responsible for any misuse or damage.

https://krazeplanet.com [200] [3416] [KrazePlanet | Offensive Security & Pentesting Experts] [38ms]

$ echo "krazeplanet.com" | vyre --stats
https://krazeplanet.com
[stats] input: 1  kept: 1  removed: 0
```

## Usage Examples
```console
# Basic probe (URL only output)
echo "dell.com" | vyre
cat subs.txt | vyre

# Full information like httpx (colored output)
cat subs.txt | vyre --status-code --title --content-length --response-time

# Fast scan with custom concurrency
cat subs.txt | vyre -c 5000

# HTTPS only with all details
cat subs.txt | vyre --only https --sc --title --rt

# Probe additional ports
cat subs.txt | vyre --ports 80,443,8080,8443

# Save to file + show stats
cat subs.txt | vyre -o results.txt --stats

# Silent mode for scripting
cat subs.txt | vyre --silent | tee results.txt

# Pipe to other tools
cat subs.txt | vyre --sc | grep '\[200\]' | nuclei -t ~/nuclei-templates/exposures/
```
