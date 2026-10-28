//! The Metrics card at the end of the General page: whether GitButler's apps send error reports
//! and usage metrics, as the desktop app's Telemetry page sets.

use gpui::Context;

use super::{Row, SettingsView};
use crate::backend::metrics;
use crate::theme::Theme;
use crate::ui::switch;

impl SettingsView {
    /// Nothing until they're read.
    pub(super) fn metrics_rows(&self, cx: &Context<Self>) -> Vec<Row> {
        let (Some(errors), Some(usage)) = (self.error_reporting, self.usage_metrics) else {
            return Vec::new();
        };
        let errors =
            switch("error-reporting", errors, Theme::get(cx)).on_click(Self::toggle_saved(
                |this| this.error_reporting.as_mut(),
                "Couldn't change error reporting",
                metrics::set_errors,
                Self::load_app_settings,
                cx,
            ));
        let usage = switch("usage-metrics", usage, Theme::get(cx)).on_click(Self::toggle_saved(
            |this| this.usage_metrics.as_mut(),
            "Couldn't change usage metrics",
            metrics::set_usage,
            Self::load_app_settings,
            cx,
        ));
        vec![
            Row::new(
                "Error reporting",
                "Reports the app's crashes and errors, which helps us catch issues more quickly.",
                errors,
            ),
            Row::new(
                "Usage metrics",
                "Shares usage statistics, strictly to help us improve the app, with no personal \
                 information: no names, paths or anything you wrote.",
                usage,
            ),
        ]
    }
}
