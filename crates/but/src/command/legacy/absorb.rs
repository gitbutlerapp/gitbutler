use crate::{
    id::{CommitId, CommitIdRef},
    theme::{self, Paint, Theme},
};
use bstr::{BStr, ByteSlice as _};
use but_ctx::Context;
use but_hunk_assignment::{
    AbsorptionTarget, CommitAbsorption, JsonAbsorbOutput, JsonCommitAbsorption, JsonFileAbsorption,
};
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use itertools::Itertools;

use crate::{
    CliId, CliResult, IdMap, bad_input,
    error::BadInput,
    id::{UncommittedHunkOrFile, parser::parse_sources},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils,
        change_source::{ChangeSourceId, InvokedFrom},
        merged_upstream::MergedUpstream,
    },
};

fn worktree_changes_cannot_be_absorbed(name: &BStr) -> BadInput {
    bad_input(format!(
        "Cannot absorb uncommitted changes in worktree {name} yet"
    ))
}

/// Amends changes into the appropriate commits where they belong.
///
/// The semantic for finding "the appropriate commit" is as follows
/// - Changes are amended into the topmost commit of the leftmost (first) lane (branch)
/// - If a change is assigned to a particular lane (branch), it will be amended into a commit there
///     - If there are no commits in this branch, a new commit is created
/// - If a change has a dependency to a particular commit, it will be amended into that particular commit
///
/// Optionally an identifier to an Uncommitted File or a Branch (stack) may be provided.
///
/// If an Uncommitted File id is provided, absorb will be performed for just that file
/// If a Branch (stack) id is provided, absorb will be performed for all changes assigned to that stack
/// If no source is provided, absorb is performed for all uncommitted changes
pub(crate) fn handle(
    ctx: &mut Context,
    out: IntermediateChannel<'_>,
    source: Option<&str>,
    dry_run: bool,
    allow_merged: crate::args::atoms::AllowMergedArg,
    invoked_from: &InvokedFrom,
) -> CliResult<AbsorbOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, guard.read_permission(), Some(invoked_from))?;
    let source: Option<CliId> = source
        .map(|s| -> CliResult<CliId> {
            // Uncommitted selectors resolve in the uncommitted namespace first
            // so later commits cannot shadow them; branch selectors resolve in
            // the full namespace. A selector that names neither is an error —
            // silently falling back to absorbing everything is never intended.
            let resolved =
                match crate::id::parser::resolve_uncommitted_part(ctx, &id_map, s) {
                    Ok(ids) if !ids.is_empty() => ids,
                    _ => parse_sources(ctx, &id_map, s)?,
                };
            let mut acceptable = resolved.into_iter().filter(|id| {
                matches!(id, CliId::UncommittedHunkOrFile { .. })
                    || matches!(id, CliId::Branch(..))
            });
            let first = acceptable.next().ok_or_else(|| {
                bad_input(format!(
                    "'{s}' does not name an uncommitted change or branch; refusing to absorb everything"
                ))
            })?;
            if acceptable.next().is_some() {
                return Err(bad_input(format!(
                    "'{s}' is ambiguous - it matches more than one uncommitted change. Use more characters to disambiguate."
                ))
                .into());
            }
            Ok(first)
        })
        .transpose()?;

    let target = if let Some(source) = source {
        match source {
            CliId::UncommittedHunkOrFile(UncommittedHunkOrFile { hunks, source, .. }) => {
                if let ChangeSourceId::Worktree(name) = source {
                    return Err(worktree_changes_cannot_be_absorbed(name.as_ref()).into());
                }
                AbsorptionTarget::Hunks {
                    hunks: hunks.map(|id_and_hunk| id_and_hunk.hunk).into(),
                }
            }
            CliId::Branch(branch) => AbsorptionTarget::Branch {
                branch_name: branch.name,
            },
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid source: expected an uncommitted file or branch"
                )
                .into());
            }
        }
    } else {
        if let ChangeSourceId::Worktree(name) = invoked_from.managed_source(&id_map)? {
            return Err(worktree_changes_cannot_be_absorbed(name.as_ref()).into());
        }
        Default::default()
    };

    // TODO: Ideally, there's a simpler way of getting the worktree changes without passing the context to it.
    // At this time, the context is passed pretty deep into the function.
    let absorption_plan =
        but_api::legacy::absorb::absorption_plan_with_perm(ctx, target, guard.write_permission())?;

    // Absorbing into a landed commit entangles the new change with an entire
    // already-merged commit, which conflicts on the next `but pull`. Drop such
    // plan entries and tell the user, instead of silently amending them.
    let merged = MergedUpstream::from_ctx(ctx, allow_merged)?;
    let (skipped, absorption_plan): (Vec<_>, Vec<_>) = absorption_plan
        .into_iter()
        .partition(|absorption| merged.contains_commit(absorption.commit_id));
    let skipped: Vec<SkippedAbsorption> = skipped
        .into_iter()
        .map(|absorption| SkippedAbsorption {
            commit_id: absorption.commit_id,
            commit_summary: absorption.commit_summary,
        })
        .collect();
    if !skipped.is_empty() && out.format().is_json() {
        // JSON output is parsed, so the warning goes to stderr.
        let ids = skipped
            .iter()
            .map(|skipped| skipped.commit_id.to_hex_with_len(7).to_string())
            .join(", ");
        eprintln!(
            "warning: skipped absorbing into {} merged-upstream commit(s): {ids}. Run `but pull` to update the workspace, or pass --allow-merged to absorb anyway.",
            skipped.len()
        );
    }
    if !skipped.is_empty() && absorption_plan.is_empty() {
        return Ok(AbsorbOutcome {
            skipped,
            result: AbsorbResult::NothingLeft,
        });
    }

    let plan = AbsorptionPlan::new(&absorption_plan, &id_map);

    if dry_run {
        return Ok(AbsorbOutcome {
            skipped,
            result: AbsorbResult::DryRun { plan },
        });
    }

    let operation = OperationKind::Absorb;
    let _snapshot = ctx
        .create_snapshot(SnapshotDetails::new(operation), guard.write_permission())
        .ok(); // Ignore errors for snapshot creation
    let total_rejected =
        but_api::legacy::absorb::absorb_with_perm(ctx, absorption_plan, guard.write_permission())?;

    Ok(AbsorbOutcome {
        skipped,
        result: AbsorbResult::Absorbed {
            plan,
            total_rejected,
        },
    })
}

/// A plan entry that was dropped because its commit is merged upstream.
struct SkippedAbsorption {
    commit_id: gix::ObjectId,
    commit_summary: String,
}

/// The absorption plan as JSON, with the display of each target commit.
struct AbsorptionPlan {
    json: JsonAbsorbOutput,
    /// The rendered commit of each entry in `json.commits`.
    commit_displays: Vec<String>,
}

impl AbsorptionPlan {
    fn new(commit_absorptions: &[CommitAbsorption], id_map: &IdMap) -> Self {
        let total_files: usize = commit_absorptions
            .iter()
            .flat_map(|c| c.hunks.iter().map(|h| &h.path))
            .unique()
            .count();
        if total_files == 0 {
            return AbsorptionPlan {
                json: JsonAbsorbOutput {
                    total_files: 0,
                    commits: vec![],
                },
                commit_displays: Vec::new(),
            };
        }

        let commits = commit_absorptions
            .iter()
            .map(|absorption| {
                let files: Vec<JsonFileAbsorption> = absorption
                    .hunks
                    .iter()
                    .map(|hunk| JsonFileAbsorption {
                        path: hunk.path.to_str_lossy().into_owned(),
                        hunks: get_hunk_ranges(hunk),
                    })
                    .collect();

                JsonCommitAbsorption {
                    commit_id: absorption.commit_id.to_hex().to_string(),
                    commit_summary: absorption.commit_summary.clone(),
                    reason: absorption.reason.clone(),
                    reason_description: absorption.reason.description().to_string(),
                    files,
                }
            })
            .collect();
        let commit_displays = commit_absorptions
            .iter()
            .map(|absorption| {
                theme::Commit(CommitIdRef {
                    commit_id: absorption.commit_id,
                    change_id: id_map
                        .change_id_ref(absorption.commit_id)
                        .map(|change_id| &change_id.change_id),
                })
                .to_string()
            })
            .collect();

        AbsorptionPlan {
            json: JsonAbsorbOutput {
                total_files,
                commits,
            },
            commit_displays,
        }
    }

    fn write_human(&self, out: &mut dyn WriteWithUtils, t: &Theme) -> std::fmt::Result {
        let total_files = self.json.total_files;
        if total_files == 0 {
            return writeln!(out, "No files to absorb");
        }
        writeln!(
            out,
            "Found {} changed file{} to absorb:",
            total_files,
            if total_files == 1 { "" } else { "s" }
        )?;
        writeln!(out)?;

        for (absorption, commit) in self.json.commits.iter().zip(&self.commit_displays) {
            writeln!(
                out,
                "Absorbed to commit: {} {}",
                commit, absorption.commit_summary
            )?;
            writeln!(out, "  ({})", t.hint.paint(&absorption.reason_description))?;

            for file in &absorption.files {
                let hunk_display = file.hunks.join(", ");
                writeln!(out, "    {} {}", file.path, t.hint.paint(&hunk_display))?;
            }
            writeln!(out)?;
        }
        Ok(())
    }
}

#[must_use]
pub(crate) struct AbsorbOutcome {
    skipped: Vec<SkippedAbsorption>,
    result: AbsorbResult,
}

enum AbsorbResult {
    /// Every plan entry targeted a commit that is merged upstream.
    NothingLeft,
    DryRun {
        plan: AbsorptionPlan,
    },
    Absorbed {
        plan: AbsorptionPlan,
        /// The number of files that could not be absorbed.
        total_rejected: usize,
    },
}

impl CliOutputHuman for AbsorbOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        t: &'static Theme,
    ) -> anyhow::Result<()> {
        let Self { skipped, result } = self;
        if !skipped.is_empty() {
            for skipped in &skipped {
                writeln!(
                    out,
                    "{}: not absorbing into {} {}: commit is merged upstream",
                    t.attention.paint("Skipped"),
                    theme::Commit(CommitId {
                        commit_id: skipped.commit_id,
                        change_id: None
                    }),
                    skipped.commit_summary,
                )?;
            }
            writeln!(
                out,
                "{}: most likely you want `but pull`, which removes landed work; in rare cases pass --allow-merged to absorb anyway",
                t.info.paint("Hint")
            )?;
        }

        match result {
            AbsorbResult::NothingLeft => {
                writeln!(out, "Nothing left to absorb")?;
            }
            AbsorbResult::DryRun { plan } => {
                plan.write_human(out, t)?;
                let message = t.success.paint("Dry run complete. No changes were made.");
                writeln!(out, "{message}")?;
            }
            AbsorbResult::Absorbed {
                plan,
                total_rejected,
            } => {
                plan.write_human(out, t)?;
                writeln!(out)?;
                if total_rejected > 0 {
                    writeln!(
                        out,
                        "{}: Failed to absorb {} file{}",
                        t.attention.paint("Warning"),
                        total_rejected,
                        if total_rejected == 1 { "" } else { "s" }
                    )?;
                }
                writeln!(
                    out,
                    "{}: you can run `but undo` to undo these changes",
                    t.info.paint("Hint")
                )?;
            }
        }
        Ok(())
    }
}

impl CliOutput for AbsorbOutcome {
    fn on_json(self) -> impl serde::Serialize {
        let Self { skipped, result } = self;
        let skipped_ids: Vec<String> = skipped
            .iter()
            .map(|skipped| skipped.commit_id.to_string())
            .collect();
        match result {
            AbsorbResult::NothingLeft => serde_json::json!({
                "ok": false,
                "skippedMergedUpstream": skipped_ids,
            }),
            AbsorbResult::DryRun { plan } => {
                serde_json::to_value(plan.json).unwrap_or(serde_json::Value::Null)
            }
            AbsorbResult::Absorbed {
                plan,
                total_rejected,
            } => {
                let mut combined = serde_json::json!({
                    "ok": total_rejected == 0,
                    "rejected": total_rejected,
                    "plan": plan.json,
                });
                if !skipped_ids.is_empty() {
                    combined["skippedMergedUpstream"] = serde_json::json!(skipped_ids);
                }
                combined
            }
        }
    }
}

fn format_hunk_range(hunk_header: &but_core::HunkHeader) -> String {
    if hunk_header.old_lines == 0 {
        // New file or added lines only
        format!("+{},{}", hunk_header.new_start, hunk_header.new_lines)
    } else if hunk_header.new_lines == 0 {
        // Deleted lines only
        format!("-{},{}", hunk_header.old_start, hunk_header.old_lines)
    } else {
        // Modified lines
        format!(
            "@{},{} +{},{}",
            hunk_header.old_start,
            hunk_header.old_lines,
            hunk_header.new_start,
            hunk_header.new_lines
        )
    }
}

fn get_hunk_ranges(hunk: &but_core::SingleHunk) -> Vec<String> {
    if let Some(hunk_header) = &hunk.hunk_header {
        vec![format_hunk_range(hunk_header)]
    } else {
        // Binary file or file too large - no hunk information
        vec!["(binary or large file)".to_string()]
    }
}
