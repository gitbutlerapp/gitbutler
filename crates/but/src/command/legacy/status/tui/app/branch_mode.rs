use anyhow::Context as _;
use but_ctx::Context;
use gix::refs::Category;
use ratatui::{style::Style, text::Span};

use crate::{
    CliId,
    command::legacy::{
        branch::{
            self,
            new::{
                NewOperation, NewStackedBranchOperation, NewStackedBranchTarget,
                NewUnstackedBranchOperation,
            },
        },
        merge::messaging,
        status::{
            output::StatusOutputLineData,
            tui::{
                Message, ReloadCause, SelectAfterReload,
                app::{
                    App, Modal,
                    mark::{Marks, MarksRef},
                },
                confirm::Confirm,
                fuzzy_picker::{Col, FuzzyPicker, FuzzyPickerItem, SearchableToken},
                key_bind::fuzzy_picker_key_binds,
                mode::Mode,
                render::{ModeRender, RenderSingleLineSpans, SpanExt as _},
                toast::ToastKind,
            },
        },
        switch::{self, SwitchBranchItem, SwitchOperation, switch_branch_items},
    },
    theme::Theme,
    utils::targeting::Side,
};

use super::MoveCursorDiration;

#[derive(Debug, Clone)]
pub struct BranchMode {
    pub marks: Marks,
    pub side: Side,
}

impl Default for BranchMode {
    fn default() -> Self {
        Self {
            marks: Default::default(),
            side: Side::Above,
        }
    }
}

impl ModeRender for BranchMode {}

impl BranchMode {
    pub fn render_insert_branch_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        is_selected: bool,
        status_line_idx: usize,
        lines_part_of_current_branch: Option<&[bool]>,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        let Some(lines_part_of_current_branch) = lines_part_of_current_branch else {
            return;
        };

        match data {
            StatusOutputLineData::UncommittedChanges { .. } | StatusOutputLineData::MergeBase => {
                if is_selected {
                    line.extend([
                        Span::raw("<< branch >>").mode_colors(&*app.mode, app.theme),
                        Span::raw(" "),
                    ]);
                }
            }
            _ => match self.side {
                Side::Above => {
                    if is_selected {
                        line.extend([
                            Span::raw("<< branch >>").mode_colors(&*app.mode, app.theme),
                            Span::raw(" "),
                        ]);
                    }
                }
                Side::Below => {
                    if line_part_of_branch(status_line_idx, lines_part_of_current_branch)
                        && !line_part_of_branch(status_line_idx + 1, lines_part_of_current_branch)
                    {
                        line.extend([
                            Span::raw("<< branch below >>").mode_colors(&*app.mode, app.theme),
                            Span::raw(" "),
                        ]);
                    }
                }
            },
        }
    }
}

fn line_part_of_branch(
    line_idx: impl Into<Option<usize>>,
    lines_part_of_current_branch: &[bool],
) -> bool {
    let Some(line_idx) = line_idx.into() else {
        return false;
    };
    lines_part_of_current_branch
        .get(line_idx)
        .copied()
        .unwrap_or(false)
}

#[derive(Debug)]
pub enum BranchMessage {
    Start,
    Switch,
    New { switch: bool },
    PickAndSwitch,
    ToggleInsertSide,
    Land,
}

impl App {
    pub fn handle_branch(
        &mut self,
        branch_message: BranchMessage,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        match branch_message {
            BranchMessage::Start => self.handle_branch_start(messages),
            BranchMessage::Switch => self.handle_branch_switch(ctx, messages)?,
            BranchMessage::New { switch } => self.handle_branch_new(ctx, messages, switch)?,
            BranchMessage::PickAndSwitch => self.handle_branch_pick_and_switch(ctx)?,
            BranchMessage::ToggleInsertSide => self.handle_branch_toggle_insert_side(),
            BranchMessage::Land => self.handle_branch_land(ctx)?,
        }

        Ok(())
    }

    fn handle_branch_start(&mut self, _messages: &mut Vec<Message>) {
        let marks = match self.mode.marks_ref() {
            marks @ (MarksRef::Empty | MarksRef::Branches { .. }) => marks,
            MarksRef::Hunks { .. } | MarksRef::Commits { .. } | MarksRef::Committed { .. } => {
                return;
            }
        };

        let new_mode = Mode::Branch(BranchMode {
            marks: marks.to_owned(),
            side: Side::Above,
        });

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = new_mode;
            });

        self.ensure_cursor_is_on_selectable_line(MoveCursorDiration::Up);
    }

    fn handle_branch_switch(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|line| line.data.cli_id())
        else {
            return Ok(());
        };

        let CliId::Branch(branch_id) = &**selection else {
            return Ok(());
        };

        let branch = Category::LocalBranch.to_full_name(&*branch_id.name)?;

        let mut guard = ctx.exclusive_worktree_access();
        let _outcome = switch::run(
            ctx,
            guard.write_permission(),
            SwitchOperation::Branch { branch },
        )?;

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                Some(SelectAfterReload::Branch(branch_id.name.clone())),
                ReloadCause::Mutation,
            ),
        ]);

        Ok(())
    }

    /// Land the marked branches — or the branch under the cursor when none are marked — directly
    /// onto the target with a single update, after a confirmation that discloses everything being
    /// published. A marked branch with segments below it lands them along with it; segments above
    /// it stay applied.
    fn handle_branch_land(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let Mode::Branch(branch_mode) = &*self.mode else {
            return Ok(());
        };
        let branch_names: Vec<String> = match &branch_mode.marks {
            Marks::Branches(branches) => branches.iter().map(|b| b.name.clone()).collect(),
            Marks::Empty => {
                let Some(selection) = self
                    .cursor
                    .selected_line(&self.status_lines)
                    .and_then(|line| line.data.cli_id())
                else {
                    return Ok(());
                };
                let CliId::Branch(branch) = &**selection else {
                    return Ok(());
                };
                vec![branch.name.clone()]
            }
            Marks::Commits(_) | Marks::Hunks(_) | Marks::Committed(_) => return Ok(()),
        };

        let target_display = {
            let mut guard = ctx.exclusive_worktree_access();
            let local_target = {
                let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
                but_api::land::ensure_landable_checkout(&ws)?;
                but_api::land::ensure_supported_target_upstream(&ws.graph.project_meta, &repo)?;
                let project_meta = &ws.graph.project_meta;
                match project_meta.remote_target_ref(&repo)? {
                    Some(_) => None,
                    None => Some(
                        project_meta
                            .target_ref_or_err()?
                            .as_local_full_name_ref()
                            .shorten()
                            .to_string(),
                    ),
                }
            };
            match local_target {
                Some(local_target) => local_target,
                None => {
                    let base_branch = but_api::legacy::virtual_branches::get_base_branch_data(
                        ctx,
                        guard.write_permission(),
                    )?
                    .context("No base branch configured")?;
                    let push_remote_name = base_branch
                        .push_remote_name
                        .filter(|name| !name.is_empty())
                        .or(base_branch.remote_name)
                        .context("The target has no remote")?;
                    format!("{push_remote_name}/{}", base_branch.short_name)
                }
            }
        };

        // Look below every branch: one sitting on other segments publishes them too, and the
        // confirmation must name what that is.
        let landings = messaging::landings(ctx, &branch_names, true)?;
        // Each mark lands with the named segments below it, so those are requested by name,
        // bottom first, and no mark needs `whole_stack` — which would also refuse any mark that
        // isn't the top of its stack. Only commits on unnamed segments can't be named.
        let mut land_names: Vec<String> = Vec::new();
        for landing in &landings {
            for name in landing
                .lower
                .segments
                .iter()
                .rev()
                .chain(std::iter::once(&landing.branch))
            {
                if !land_names.contains(name) {
                    land_names.push(name.clone());
                }
            }
        }
        let whole_stack = landings
            .iter()
            .any(|landing| landing.lower.unnamed_commits > 0);
        let warning = messaging::direct_target_update_warning(ctx, &landings, &target_display)?;

        // The tips the user is asked about: the land refuses if a branch moves before it's decided,
        // as when an agent commits while the confirmation is open.
        let tips = {
            let _guard = ctx.exclusive_worktree_access();
            let repo = ctx.repo.get()?;
            land_names
                .iter()
                .map(|branch_name| {
                    let mut reference = repo
                        .try_find_reference(&format!("refs/heads/{branch_name}"))?
                        .with_context(|| format!("Branch {branch_name} not found"))?;
                    anyhow::Ok(reference.peel_to_id()?.detach())
                })
                .collect::<anyhow::Result<Vec<_>>>()?
        };

        let body = format!(
            "{warning}\n\nLand {} onto {target_display}?",
            branch_names.join(", ")
        );
        let branch_names = land_names;

        let confirm = Confirm::new_wrapped(body, self.theme, move |ctx, messages| {
            let result = but_api::land::branch_land(
                ctx,
                branch_names.clone(),
                false,
                false,
                whole_stack,
                tips.clone(),
            )?;
            let text = match result.landed {
                but_api::land::BranchLandKind::AlreadyIntegrated => {
                    format!("Already on {target_display}: {}", branch_names.join(", "))
                }
                but_api::land::BranchLandKind::Updated { .. } => {
                    let landed: Vec<&str> = branch_names
                        .iter()
                        .filter(|name| !result.already_integrated.contains(name))
                        .map(String::as_str)
                        .collect();
                    format!("Landed {} onto {target_display}", landed.join(", "))
                }
            };
            messages.push(Message::ShowToast {
                kind: ToastKind::Info,
                text: text.into(),
            });
            if !result.deleted_remote_branches.is_empty() {
                messages.push(Message::ShowToast {
                    kind: ToastKind::Info,
                    text: format!(
                        "Deleted the remote copies of {}",
                        result.deleted_remote_branches.join(", ")
                    )
                    .into(),
                });
            }
            if result.reconcile_skipped {
                messages.push(Message::ShowToast {
                    kind: ToastKind::Error,
                    text: "The remaining branches were not updated onto the new target. Run \
                           `but pull` to finish."
                        .into(),
                });
            }
            messages.extend([
                Message::ClearMarks,
                Message::EnterNormalModeAfterConfirmingOperation,
                Message::Reload(None, ReloadCause::Mutation),
            ]);
            Ok(())
        });
        self.modal = Some(Modal::Confirm { confirm });

        Ok(())
    }

    fn handle_branch_toggle_insert_side(&mut self) {
        let Mode::Branch(branch_mode) = self
            .mode
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        else {
            return;
        };
        branch_mode.side = branch_mode.side.toggle();
    }

    fn handle_branch_pick_and_switch(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let selected_branch = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|line| line.data.cli_id())
            .and_then(|id| {
                if let CliId::Branch(branch) = &**id {
                    Some(branch.name.as_str())
                } else {
                    None
                }
            });

        let items = {
            let guard = ctx.shared_worktree_access();
            switch_branch_items(ctx, guard.read_permission(), selected_branch, true)?
        };
        let Some(items) = items else {
            return Ok(());
        };

        let picker = FuzzyPicker::new(items, self.theme, |item, ctx, messages| {
            let what_to_select = match item {
                SwitchBranchItem::Branch { name, .. } => {
                    let branch = Category::LocalBranch.to_full_name(&*name)?;

                    let mut guard = ctx.exclusive_worktree_access();
                    _ = switch::run(
                        ctx,
                        guard.write_permission(),
                        SwitchOperation::Branch { branch },
                    )?;

                    SelectAfterReload::Branch(name)
                }
                SwitchBranchItem::Workspace => {
                    let mut guard = ctx.exclusive_worktree_access();
                    _ = switch::run(ctx, guard.write_permission(), SwitchOperation::Workspace)?;

                    SelectAfterReload::Uncommitted
                }
            };

            messages.extend([
                Message::EnterNormalModeAfterConfirmingOperation,
                Message::Reload(Some(what_to_select), ReloadCause::Mutation),
            ]);

            Ok(())
        });

        self.modal = Some(Modal::SwitchBranchPicker {
            picker: Box::new(picker),
            key_binds: fuzzy_picker_key_binds(),
        });

        Ok(())
    }

    fn handle_branch_new(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
        switch: bool,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        let side = if let Mode::Branch(branch_mode) = &*self.mode {
            branch_mode.side
        } else {
            Side::Above
        };

        let new_name = match &selection.data {
            StatusOutputLineData::Branch { cli_id, .. } => {
                let CliId::Branch(branch) = &**cli_id else {
                    return Ok(());
                };

                let mut guard = ctx.exclusive_worktree_access();
                let mut meta = ctx.meta()?;

                let outcome = branch::new::run(
                    ctx,
                    &mut meta,
                    guard.write_permission(),
                    NewOperation::NewStackedBranch(NewStackedBranchOperation {
                        name: None,
                        target: NewStackedBranchTarget::Branch(
                            Category::LocalBranch.to_full_name(&*branch.name)?,
                        ),
                        side,
                        switch,
                    }),
                )?;

                outcome.name.shorten().to_string()
            }
            StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::WorktreeUncommitted { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UncommittedFile { .. } => {
                let mut guard = ctx.exclusive_worktree_access();
                let mut meta = ctx.meta()?;

                let outcome = branch::new::run(
                    ctx,
                    &mut meta,
                    guard.write_permission(),
                    NewOperation::NewUnstackedBranch(NewUnstackedBranchOperation {
                        name: None,
                        switch,
                    }),
                )?;

                outcome.name.shorten().to_string()
            }
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::Commit { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return Ok(()),
        };

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                Some(SelectAfterReload::Branch(new_name)),
                ReloadCause::Mutation,
            ),
        ]);

        Ok(())
    }
}

impl FuzzyPickerItem for SwitchBranchItem {
    fn columns(&self, searchable: SearchableToken) -> impl IntoIterator<Item = Col<'_>> {
        match self {
            SwitchBranchItem::Branch {
                name,
                updated_at_display,
                updated_at: _,
            } => [
                Col {
                    text: name.as_str().into(),
                    searchable: Some(searchable),
                },
                Col {
                    text: updated_at_display.as_str().into(),
                    searchable: None,
                },
            ],
            SwitchBranchItem::Workspace => [
                Col {
                    text: "workspace".into(),
                    searchable: Some(searchable),
                },
                Col {
                    text: "".into(),
                    searchable: None,
                },
            ],
        }
    }

    fn style(&self, theme: &Theme) -> Style {
        match self {
            SwitchBranchItem::Branch { .. } => theme.local_branch,
            SwitchBranchItem::Workspace => theme.info,
        }
    }

    fn secondary_style(&self, theme: &Theme) -> Style {
        theme.hint
    }
}
