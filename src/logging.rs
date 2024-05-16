use tracing::level_filters::LevelFilter;
use tracing_subscriber::{filter, prelude::*};
pub fn log_init() {
    let stdout_layer = tracing_subscriber::fmt::layer()
        // .pretty()
        .with_filter(LevelFilter::DEBUG);

    // Create a rolling file appender
    let file_appender = tracing_appender::rolling::Builder::new()
        .filename_suffix("johnson_rs.log.json")
        .max_log_files(7)
        .build("./logs")
        .expect("Johnson should be able to create rolling file appender");

    // Create a subscriber layer that will output to a file
    let f_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .json()
        .with_writer(file_appender)
        .with_filter(LevelFilter::DEBUG);

    tracing_subscriber::registry()
        .with(
            stdout_layer
                .and_then(f_layer)
                .with_filter(filter::filter_fn(|metadata| {
                    metadata.target().contains("johnson") // Only log Johnson Bot logs
                })),
        )
        .init();
}
