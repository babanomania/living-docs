use tracing_subscriber::EnvFilter;

/// Quiet by default (warnings only); `--verbose` raises the level to debug.
/// `RUST_LOG` always wins when set, for ad hoc debugging.
pub fn init(verbose: bool) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(if verbose { "debug" } else { "warn" }));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}
