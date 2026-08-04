use but_testsupport::Sandbox;
use ratatui::{Terminal, backend::TestBackend};

use crate::{
    args::OutputFormat,
    command::legacy::status::{
        StatusFlags, StatusOutput, StatusRenderMode, TuiLaunchOptions, TuiOutcome, TuiRunOptions,
        build_status_context, build_status_output, resolve_tui_target, status_flags_for_tui_target,
        tui::{App, BackstackEntry, Message, ReloadCause, app::UpdateContext},
    },
    tui::{
        Clipboard,
        event_polling::EventPolling,
        test_utils::{TestTui, TestTuiInputThenRenderResult, configure_test_repo},
    },
    utils::OutputChannel,
};

use super::super::{mode::Mode, render::status_layout};

pub struct TestTuiOptions {
    pub width: u16,
    pub height: u16,
    pub run_options: TuiRunOptions,
    pub show_file_browser: bool,
    pub launch_options: TuiLaunchOptions,
}

impl Default for TestTuiOptions {
    fn default() -> Self {
        Self {
            width: 100,
            height: 20,
            run_options: Default::default(),
            show_file_browser: false,
            launch_options: Default::default(),
        }
    }
}

pub fn test_status_tui(env: Sandbox) -> TestTui<App> {
    test_status_tui_with_options(env, TestTuiOptions::default())
}

pub fn test_status_tui_with_options(env: Sandbox, options: TestTuiOptions) -> TestTui<App> {
    let TestTuiOptions {
        width,
        height,
        run_options,
        show_file_browser,
        launch_options,
    } = options;

    configure_test_repo(&env);
    let mut ctx = env.context();
    let operating_mode = but_api::legacy::modes::operating_mode(&ctx)
        .expect("failed to get operating mode")
        .operating_mode;
    let mut out = OutputChannel::new(OutputFormat::Human { agent: false });

    let flags = StatusFlags::all_false();

    let mut guard = ctx.exclusive_worktree_access();

    let format = out.format();
    let mut status_ctx = build_status_context(
        &mut ctx,
        guard.write_permission(),
        &mut out,
        format,
        &operating_mode,
        flags,
        StatusRenderMode::Tui(launch_options.clone()),
    )
    .expect("failed to build status context");
    let initial_target = resolve_tui_target(
        &ctx.repo.get().unwrap(),
        &status_ctx.id_map,
        &launch_options,
    )
    .expect("failed to resolve TUI target");
    status_ctx.flags = status_flags_for_tui_target(status_ctx.flags, initial_target.as_ref());
    let mut lines = Vec::new();
    let mut status_output = StatusOutput::Buffer { lines: &mut lines };
    build_status_output(&ctx, &status_ctx, &mut status_output)
        .expect("failed to build status output");

    let incoming_out_of_band_messages = Vec::new();
    let head_sha = super::super::operations::head_sha(&mut ctx).expect("failed to read HEAD");

    let (clipboard, clipboard_text) = Clipboard::test();

    let app = App::new(
        &ctx,
        lines,
        status_ctx.flags,
        launch_options,
        initial_target,
        run_options,
        show_file_browser,
        incoming_out_of_band_messages,
        head_sha,
        clipboard,
        operating_mode,
    )
    .unwrap();

    let terminal =
        Terminal::new(TestBackend::new(width, height)).expect("failed to create test terminal");

    TestTui::new(
        app,
        ctx,
        terminal,
        env,
        out,
        width,
        height,
        None,
        clipboard_text,
    )
}

impl TestTui<App> {
    #[track_caller]
    pub fn reload(&mut self) -> TestTuiInputThenRenderResult<'_, App> {
        self.render_with_messages(
            None,
            Vec::from([Message::Reload(None, ReloadCause::Mutation)]),
        )
    }

    #[track_caller]
    pub fn recreate(mut self) -> Self {
        let env = self.take_env();
        self = test_status_tui_with_options(
            env,
            TestTuiOptions {
                width: self.width(),
                height: self.height(),
                ..Default::default()
            },
        );
        self
    }

    /// Lower level utility method that generally shouldn't be used. Prefer [`TestTui::input`] or
    /// [`TestTui::reload`] instead.
    #[doc(hidden)]
    #[track_caller]
    pub fn render_with_messages<E>(
        &mut self,
        event: E,
        messages: Vec<Message>,
    ) -> TestTuiInputThenRenderResult<'_, App>
    where
        E: EventPolling,
    {
        let mut ctx = self.env().context();
        let mut update_ctx = UpdateContext {
            messages,
            other_messages: Vec::new(),
            ctx: &mut ctx,
        };

        self.render_with_update_context(event, &mut update_ctx)
    }
}

impl TestTuiInputThenRenderResult<'_, App> {
    /// We might not be able to find the selected row for example if we're in full screen details
    /// view, where the status cursor exists but the status list is not rendered.
    fn selected_status_row(&self) -> Option<u16> {
        if matches!(&*self.app().mode, Mode::Details(details_mode) if details_mode.full_screen) {
            return None;
        }

        let buffer = self.terminal().backend().buffer();
        let terminal_area = *buffer.area();
        let main_content_area = ratatui::layout::Rect {
            height: terminal_area.height.saturating_sub(1),
            ..terminal_area
        };
        let status_inner_area = status_layout(self.app(), main_content_area).status_area;

        let cursor_index = self.app().cursor.index();
        let scroll_top = self.app().status_scroll.top();
        if cursor_index < scroll_top {
            return None;
        }

        let row_offset = cursor_index - scroll_top;
        if row_offset >= status_inner_area.height as usize {
            return None;
        }

        Some(status_inner_area.y + row_offset as u16)
    }

    #[track_caller]
    pub fn assert_current_line_eq(self, expected: impl snapbox::IntoData) -> Self {
        let backend = self.terminal().backend();
        let buffer = backend.buffer();
        let area = *buffer.area();

        let selected_row = self
            .selected_status_row()
            .expect("failed to find selected status row in rendered output");

        let mut line = String::new();
        for x in area.x..area.x.saturating_add(area.width) {
            line.push_str(buffer[(x, selected_row)].symbol());
        }
        let line = line.trim_end();

        let actual = snapbox::IntoData::into_data(line);
        let actual = actual.render().expect("current line should render as text");

        let expected = snapbox::IntoData::into_data(expected);

        snapbox::assert_data_eq!(actual, expected);

        self
    }

    pub fn take_outcome(mut self) -> Option<TuiOutcome> {
        self.app_mut().outcome.take()
    }

    #[track_caller]
    pub fn assert_backstack_eq(self, entries: impl IntoIterator<Item = BackstackEntry>) -> Self {
        let expected = entries.into_iter().collect::<Vec<_>>();
        let actual = self.app().backstack.iter().copied().collect::<Vec<_>>();
        if expected != actual {
            panic!("wrong backstack\n  expected: {expected:?}\n  actual: {actual:?}");
        }
        self
    }

    pub fn assert_marks_count_eq(self, count: usize) -> Self {
        assert_eq!(self.app().marks_ref().len(), count);
        self
    }
}
