#[cfg(unix)]
mod entry;

#[cfg(unix)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    entry::main().await?;
    Ok(())
}

#[cfg(windows)]
fn main() {}
