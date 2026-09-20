use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init(otlp_endpoint: Option<&str>) -> Result<Option<SdkTracerProvider>, String> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("frontier_server=info"));
    let format_layer = tracing_subscriber::fmt::layer().with_target(true);

    let Some(endpoint) = otlp_endpoint else {
        tracing_subscriber::registry()
            .with(filter)
            .with(format_layer)
            .init();
        return Ok(None);
    };

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .map_err(|error| format!("could not build OTLP exporter: {error}"))?;
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();
    let tracer = provider.tracer("frontier-server");
    opentelemetry::global::set_tracer_provider(provider.clone());

    tracing_subscriber::registry()
        .with(filter)
        .with(format_layer)
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    Ok(Some(provider))
}

pub fn shutdown(provider: Option<SdkTracerProvider>) {
    if let Some(provider) = provider {
        if let Err(error) = provider.shutdown() {
            tracing::error!(%error, "could not shut down OpenTelemetry provider");
        }
    }
}
