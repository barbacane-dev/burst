use crate::config::TelemetryConfig;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::Sampler;

/// Initialise an OpenTelemetry tracer exporting via OTLP.
///
/// Returns `None` when `otlp_endpoint` is absent or empty (tracing disabled).
/// The caller must hold the returned provider alive for the process lifetime
/// and call [`shutdown`] during graceful shutdown to flush buffered spans.
pub fn init_tracer(
    config: &TelemetryConfig,
) -> Option<opentelemetry_sdk::trace::SdkTracerProvider> {
    let endpoint = config.otlp_endpoint.as_deref().filter(|s| !s.is_empty())?;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .ok()?;

    let sampler = if config.trace_sample_rate >= 1.0 {
        Sampler::AlwaysOn
    } else if config.trace_sample_rate <= 0.0 {
        Sampler::AlwaysOff
    } else {
        Sampler::TraceIdRatioBased(config.trace_sample_rate)
    };

    let resource = Resource::builder_empty().with_service_name("burst").build();

    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_sampler(sampler)
        .with_resource(resource)
        .build();

    Some(provider)
}

/// Flush buffered spans and shut down the tracer provider.
pub fn shutdown(provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>) {
    if let Some(provider) = provider
        && let Err(e) = provider.shutdown()
    {
        tracing::warn!("failed to shut down tracer provider: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TelemetryConfig;

    #[test]
    fn disabled_when_no_endpoint() {
        let config = TelemetryConfig::default();
        assert!(init_tracer(&config).is_none());
    }

    #[test]
    fn disabled_when_empty_endpoint() {
        let config = TelemetryConfig {
            otlp_endpoint: Some(String::new()),
            trace_sample_rate: 1.0,
        };
        assert!(init_tracer(&config).is_none());
    }
}
