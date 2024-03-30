use crate::error::gb::{ErrorCode, ErrorContext};
use sentry::{
    protocol::{value::Map, Event, Exception, Value},
    types::Uuid,
};
use std::collections::BTreeMap;

pub trait SentrySender {
    fn send_to_sentry(self) -> Uuid;
}

impl<E: Into<ErrorContext>> SentrySender for E {
    fn send_to_sentry(self) -> Uuid {
        let sentry_event = self.into().into();
        sentry::capture_event(sentry_event)
    }
}

trait PopulateException {
    fn populate_exception(
        self,
        exceptions: &mut Vec<Exception>,
        vars: &mut BTreeMap<String, Value>,
    );
}

impl PopulateException for ErrorContext {
    fn populate_exception(
        self,
        exceptions: &mut Vec<Exception>,
        vars: &mut BTreeMap<String, Value>,
    ) {
        let (error, mut context) = self.into_owned();

        let mut exc = Exception {
            ty: error.code(),
            value: Some(error.message()),
            ..Exception::default()
        };

        if let Some(cause) = context.caused_by {
            cause.populate_exception(exceptions, vars);
        }

        // We don't resolve at capture time because it can DRASTICALLY
        // slow down the application (can take up to 0.5s to resolve
        // a *single* frame). We do it here, only when a Sentry event
        // is being created.
        context.backtrace.resolve();
        exc.stacktrace =
            sentry::integrations::backtrace::backtrace_to_stacktrace(&context.backtrace);

        ::backtrace::clear_symbol_cache();

        vars.insert(
            error.code(),
            Value::Object(
                context
                    .vars
                    .into_iter()
                    .map(|(k, v)| (k, v.into()))
                    .collect(),
            ),
        );
        exceptions.push(exc);
    }
}

impl From<ErrorContext> for Event<'_> {
    fn from(error_context: ErrorContext) -> Self {
        let mut sentry_event = Event {
            message: Some(format!(
                "{}: {}",
                error_context.error().code(),
                error_context.error().message()
            )),
            ..Event::default()
        };

        let mut vars = BTreeMap::new();
        error_context.populate_exception(&mut sentry_event.exception.values, &mut vars);

        sentry_event
            .extra
            .insert("context_vars".into(), Value::Object(Map::from_iter(vars)));

        sentry_event
    }
}
