use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Number of GPIOs to monitor
    #[arg(short, long, default_value_t = 256)]
    count: usize,
}

#[cfg(any(unix, debug_assertions))]
mod entry;

#[cfg(any(unix, debug_assertions))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let args = Args {
        count: if args.count % 8 == 0 {
            args.count
        } else {
            args.count + (8 - args.count % 8)
        },
    };

    entry::main(args).await?;
    Ok(())
}

#[cfg(all(windows, not(debug_assertions)))]
fn main() {}
