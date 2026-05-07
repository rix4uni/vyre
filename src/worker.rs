use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use futures::StreamExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio_stream::wrappers::LinesStream;

use crate::cli::Args;
use crate::output::{format_result, DisplayConfig};
use crate::prober::{probe_domain, ProbeConfig};

/// Build the shared reqwest client with all performance settings applied.
fn build_client(args: &Args) -> Result<reqwest::Client> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(args.timeout))
        // Accept self-signed, expired, and mismatched TLS certs.
        .danger_accept_invalid_certs(true)
        // Never follow redirects — report raw status codes.
        .redirect(reqwest::redirect::Policy::none())
        // Each subdomain is unique; idle connections waste memory.
        .pool_max_idle_per_host(0)
        // Disable Nagle — send packets immediately.
        .tcp_nodelay(true)
        .build()?;
    Ok(client)
}

/// Read domains from stdin, probe them with bounded concurrency, write results.
///
/// Design:
///   stdin → LinesStream → map(probe_domain) → buffer_unordered(N) → output channel
///
/// `buffer_unordered(N)` keeps exactly N probe futures in flight at all times.
/// Memory usage is O(N), not O(total_domains), so 1 billion-domain inputs are fine.
///
/// A dedicated writer task receives formatted lines over a channel and writes
/// them sequentially to stdout (and optionally a file), preventing interleaving.
pub async fn run(args: Arc<Args>) -> Result<()> {
    let ports = Arc::new(args.parsed_ports());
    let client = Arc::new(build_client(&args)?);
    let cfg = Arc::new(ProbeConfig {
        fetch_title: args.title,
        fetch_content_length: args.content_length,
    });
    let disp = Arc::new(DisplayConfig {
        show_status: args.show_status,
        show_rt:     args.show_rt,
    });

    let domains_read = Arc::new(AtomicU64::new(0));
    let urls_alive   = Arc::new(AtomicU64::new(0));

    // Unbounded channel: probers → writer.
    // Bounded would create backpressure; unbounded lets probers run freely.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    // Spawn a dedicated writer task so stdout writes are never concurrent.
    let output_path = args.output.clone();
    let writer = tokio::spawn(async move {
        let mut file: Option<tokio::fs::File> = if let Some(ref path) = output_path {
            tokio::fs::File::create(path).await.ok()
        } else {
            None
        };

        let mut stdout = tokio::io::stdout();

        while let Some(line) = rx.recv().await {
            let bytes = line.as_bytes();
            let _ = stdout.write_all(bytes).await;
            if let Some(ref mut f) = file {
                let _ = f.write_all(bytes).await;
            }
        }

        // Flush any remaining buffered data before the process exits.
        let _ = stdout.flush().await;
        if let Some(ref mut f) = file {
            let _ = f.flush().await;
        }
    });

    // Convert async stdin into a Stream<Item = String> of domain names.
    let reader = tokio::io::BufReader::new(tokio::io::stdin());
    let lines_stream = LinesStream::new(reader.lines());

    let concurrency = args.concurrency;
    let only = args.only.clone();

    let domains_read_fm = domains_read.clone();
    lines_stream
        // Drop I/O errors and blank lines silently.
        .filter_map(move |line| {
            futures::future::ready(match line {
                Ok(l) => {
                    let d = l.trim().to_string();
                    if d.is_empty() {
                        None
                    } else {
                        domains_read_fm.fetch_add(1, Ordering::Relaxed);
                        Some(d)
                    }
                }
                Err(_) => None,
            })
        })
        // Map each domain name to a future that probes it.
        .map(|domain| {
            let client = client.clone();
            let ports = ports.clone();
            let only = only.clone();
            let tx = tx.clone();
            let cfg = cfg.clone();
            let disp = disp.clone();
            let urls_alive = urls_alive.clone();

            async move {
                let results =
                    probe_domain(&client, &domain, &ports, only.as_deref(), &cfg).await;

                for result in results {
                    urls_alive.fetch_add(1, Ordering::Relaxed);
                    // Ignore send errors — writer task may have exited early.
                    let _ = tx.send(format_result(&result, &disp));
                }
            }
        })
        // Keep exactly `concurrency` futures running simultaneously.
        .buffer_unordered(concurrency)
        .for_each(|_| async {})
        .await;

    // Drop our sender so the writer task knows there are no more results.
    drop(tx);

    // Wait for the writer to flush everything before we return.
    let _ = writer.await;

    if args.stats {
        let input   = domains_read.load(Ordering::Relaxed);
        let alive   = urls_alive.load(Ordering::Relaxed);
        let dead    = input.saturating_sub(alive);
        eprintln!("[stats] input: {}  alive: {}  dead: {}", input, alive, dead);
    }

    Ok(())
}
