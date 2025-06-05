use crate::category::PANIC;
use crate::layer::PLATFORM;
use crate::tracing::FlatJsonLayer;
use ::tracing::level_filters::LevelFilter;
use std::panic::PanicHookInfo;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

// From tracing_panic library. Adapted to allow us to add our own layer + category fields.
fn panic_hook(panic_info: &PanicHookInfo) {
    let payload = panic_info.payload();

    #[allow(clippy::manual_map)]
    let payload = if let Some(s) = payload.downcast_ref::<&str>() {
        Some(&**s)
    } else if let Some(s) = payload.downcast_ref::<String>() {
        Some(s.as_str())
    } else {
        None
    };

    let location = panic_info.location().map(|l| l.to_string());

    tracing::error!(
        layer = PLATFORM,
        category = PANIC,
        panic.payload = payload,
        panic.location = location,
        "A panic occurred",
    );
}

pub fn setup_tracing(console_tracing: bool) {
    std::panic::set_hook(Box::new(panic_hook));

    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        .from_env_lossy();

    if console_tracing {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::registry()
            .with(FlatJsonLayer {})
            .with(env_filter)
            .init()
    }
}
