#[cfg(unix)]
use nix::{
    sys::resource::{Usage, UsageWho, getrusage},
    sys::time::TimeValLike,
};

macro_rules! trace_phase {
    ($name:literal $(, $($fields:tt)*)?) => {
        $crate::trace::Phase::start(tracing::trace_span!(
            $name,
            user_ms = tracing::field::Empty,
            system_ms = tracing::field::Empty
            $(, $($fields)*)?
        ))
    };
}
pub(crate) use trace_phase;

/// Let the subscriber report elapsed time and add process CPU totals to the same span.
#[must_use]
pub(crate) struct Phase {
    _span: tracing::span::EnteredSpan,
    #[cfg(unix)]
    cpu_at_start: Option<Usage>,
}

impl Phase {
    pub(crate) fn start(span: tracing::Span) -> Self {
        let span = span.entered();
        Self {
            #[cfg(unix)]
            cpu_at_start: if span.is_disabled() {
                None
            } else {
                getrusage(UsageWho::RUSAGE_SELF).ok()
            },
            _span: span,
        }
    }
}

#[cfg(unix)]
impl Drop for Phase {
    fn drop(&mut self) {
        if let Some(start) = self.cpu_at_start
            && let Ok(end) = getrusage(UsageWho::RUSAGE_SELF)
        {
            self._span
                .record(
                    "user_ms",
                    (end.user_time() - start.user_time()).num_microseconds() as f64 / 1000.0,
                )
                .record(
                    "system_ms",
                    (end.system_time() - start.system_time()).num_microseconds() as f64 / 1000.0,
                );
        }
    }
}
