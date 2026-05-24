#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("meridian-portal starting");
    if let Err(e) = meridian_portal::run().await {
        tracing::error!("portal failed: {e}");
        std::process::exit(1);
    }
}
