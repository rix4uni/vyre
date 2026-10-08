mod cli;
mod matcher;
mod output;
mod prober;
mod worker;

use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Arc::new(cli::parse());

    if !args.silent {
        let banner = output::banner();
        eprint!("{}", if args.no_color { output::strip_ansi(&banner) } else { banner });
    }

    worker::run(args).await
}
