mod cli;
mod output;
mod prober;
mod worker;

use std::sync::Arc;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Arc::new(cli::Args::parse());

    if !args.silent {
        eprint!("{}", output::banner());
    }

    worker::run(args).await
}
