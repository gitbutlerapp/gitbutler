use std::sync::Arc;

use governor::{
    clock::QuantaClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use nonzero_ext::nonzero;
use once_cell::sync::OnceCell;
use sentry::ClientInitGuard;
use sentry_tracing::SentryLayer;
use tracing::Subscriber;
use tracing_subscriber::registry::LookupSpan;

use gitbutler_core::users;

static SENTRY_QUOTA: Quota = Quota::per_second(nonzero!(1_u32)); // 1 per second at most.
static SENTRY_LIMIT: OnceCell<RateLimiter<NotKeyed, InMemoryState, QuantaClock>> = OnceCell::new();

/// Should be called once on application startup, and the returned guard should be kept alive for
/// the lifetime of the application.
pub fn init(name: &str, version: String) -> ClientInitGuard {
    sentry::init(("https://9d407634d26b4d30b6a42d57a136d255@o4504644069687296.ingest.sentry.io/4504649768108032", sentry::ClientOptions {
        environment: Some(match name {
            "GitButler" => "production",
            "GitButler Nightly" => "nightly",
            "GitButler Dev" => "development",
            _ => "unknown",
        }.into()),
        release: Some(version.into()),
        before_send: Some({
            Arc::new(|event| SENTRY_LIMIT.get_or_init(|| RateLimiter::direct(SENTRY_QUOTA)).check().is_ok().then_some(event))}),
        attach_stacktrace: true,
        traces_sample_rate: match name {
            "GitButler Dev" | "GitButler Nightly" => 0.2_f32,
            _ => 0.05_f32,
        },
        default_integrations: true,
        ..Default::default()
    }))
}

/// Sets the current user in the Sentry scope.
/// There is only one scope in the application, so this will overwrite any previous user.
pub fn configure_scope(user: Option<&users::User>) {
    let name = match user {
        Some(user) => match &user.name {
            Some(name) => Some(name.clone()),
            None => user.given_name.as_ref().cloned(),
        },
        None => None,
    };

    sentry::configure_scope(|scope| {
        scope.set_user(user.map(|user| sentry::User {
            id: Some(user.id.to_string()),
            username: name,
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
