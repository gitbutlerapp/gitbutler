use std::sync::Arc;

use sentry::ClientInitGuard;
use sentry_tracing::SentryLayer;
use tauri::PackageInfo;
use tracing::Subscriber;
use tracing_subscriber::registry::LookupSpan;

use crate::users;

/// Should be called once on application startup, and the returned guard should be kept alive for
/// the lifetime of the application.
pub fn init(package_info: &PackageInfo) -> ClientInitGuard {
    sentry::init(("https://9d407634d26b4d30b6a42d57a136d255@o4504644069687296.ingest.sentry.io/4504649768108032", sentry::ClientOptions {
        environment: Some(match package_info.name.as_str() {
            "GitButler" => "production",
            "GitButler Nightly" => "nightly",
            "GitButler Dev" => "development",
            _ => "unknown",
        }.into()),
        release: Some(package_info.version.to_string().into()),
        before_send: Some(Arc::new(|event| {
            Some(event)
        })),
        attach_stacktrace: true,
        default_integrations: true,
        ..Default::default()
    }))
}

/// Sets the current user in the Sentry scope.
/// There is only one scope in the application, so this will overwrite any previous user.
pub fn configure_scope(user: Option<&users::User>) {
    sentry::configure_scope(|scope| {
        scope.set_user(user.map(|user| sentry::User {
            id: Some(user.id.to_string()),
            username: Some(user.name.clone()),
            email: Some(user.email.clone()),
            ..Default::default()
        }));
    });
}

/// Returns a tracing layer that will send all errors to Sentry.
pub fn tracing_layer<S>() -> SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    sentry_tracing::layer().event_filter(|md| match md.level() {
        &tracing::Level::ERROR => sentry_tracing::EventFilter::Event,
        _ => sentry_tracing::EventFilter::Ignore,
    })
}
