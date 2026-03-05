use bstr::BStr;
use but_ctx::Context;

use crate::{CliId, IdMap, utils::OutputChannel};

pub(crate) fn parse_sources(
    ctx: &mut Context,
    id_map: &IdMap,
    source: &str,
) -> anyhow::Result<Vec<CliId>> {
    // Check if it's a list (contains ',')
    if source.contains(',') {
        return parse_list(ctx, id_map, source);
    }

    // Check if it's a valid range (e.g., "g0-h2" where both sides are uncommitted files).
    // If the string contains '-' but isn't a valid range (e.g., a filename like "my-file.rs"
    // or a branch name like "feature-auth"), fall through to single-entity parsing.
    if source.contains('-')
        && let Some(range_result) = try_parse_range(ctx, id_map, source)?
    {
        return Ok(range_result);
    }

    // Single source (including strings with dashes that aren't valid ranges)
    let source_result = id_map.parse_using_context(source, ctx)?;
    if source_result.len() != 1 {
        if source_result.is_empty() {
            return Err(anyhow::anyhow!(
                "Source '{source}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
            ));
        } else {
            let matches: Vec<String> = source_result
                .iter()
                .map(|id| format!("{} ({})", id.to_short_string(), id.kind_for_humans()))
                .collect();
            return Err(anyhow::anyhow!(
                "Source '{}' is ambiguous. Matches: {}. Try using more characters, a longer SHA, or the full branch name to disambiguate.",
                source,
                matches.join(", ")
            ));
        }
    }
    Ok(vec![source_result[0].clone()])
}

/// Tries to parse `source` as a range expression like "g0-h2".
///
/// A range is only valid when:
/// - The string splits on '-' into exactly 2 parts
/// - Both parts resolve to exactly one `Uncommitted` entity each
///
/// Returns `Ok(Some(ids))` for a valid range, `Ok(None)` if it's not a range
/// (allowing the caller to fall through to single-entity parsing), or `Err`
/// if it looks like a range but the IDs aren't in the display order.
fn try_parse_range(
    ctx: &mut Context,
    id_map: &IdMap,
    source: &str,
) -> anyhow::Result<Option<Vec<CliId>>> {
    let parts: Vec<&str> = source.split('-').collect();
    if parts.len() != 2 {
        return Ok(None);
    }

    // If either half fails to parse (e.g., single character "a" in "a-file.txt"),
    // this isn't a range — fall through to single-entity parsing.
    let Ok(start_matches) = id_map.parse_using_context(parts[0], ctx) else {
        return Ok(None);
    };
    let Ok(end_matches) = id_map.parse_using_context(parts[1], ctx) else {
        return Ok(None);
    };

    // Both sides must resolve to exactly one Uncommitted entity
    if start_matches.len() != 1 || end_matches.len() != 1 {
        return Ok(None);
    }
    if !matches!(&start_matches[0], CliId::Uncommitted(_))
        || !matches!(&end_matches[0], CliId::Uncommitted(_))
    {
        return Ok(None);
    }

    // Valid range — resolve positions in display order
    let all_files = get_all_files_in_display_order(ctx, id_map)?;
    let start_pos = all_files.iter().position(|id| id == &start_matches[0]);
    let end_pos = all_files.iter().position(|id| id == &end_matches[0]);

    match (start_pos, end_pos) {
        (Some(s), Some(e)) if s <= e => Ok(Some(all_files[s..=e].to_vec())),
        (Some(s), Some(e)) => Ok(Some(all_files[e..=s].to_vec())),
        _ => Err(anyhow::anyhow!(
            "Could not find range from '{}' to '{}' in the displayed file list",
            parts[0],
            parts[1]
        )),
    }
}

fn get_all_files_in_display_order(ctx: &mut Context, id_map: &IdMap) -> anyhow::Result<Vec<CliId>> {
    // First, files assigned to branches (they appear first in status display),
    // then unassigned files (they appear last in status display)
    let (_guard, _, workspace, _) = ctx.workspace_and_db()?;
    let stack_ids: Vec<Option<but_core::Id<'S'>>> =
        workspace.stacks.iter().map(|stack| stack.id).collect();
    let mut positioned_files: Vec<(usize, &BStr, CliId)> = id_map
        .uncommitted_files
        .values()
        .flat_map(|uncommitted_file| {
            let position = match uncommitted_file.stack_id() {
                Some(stack_id) => stack_ids.iter().position(|e| *e == Some(stack_id))?,
                None => usize::MAX,
            };
            Some((position, uncommitted_file.path(), uncommitted_file.to_id()))
        })
        .collect();
    positioned_files.sort_by(|(a_pos, a_path, _), (b_pos, b_path, _)| {
        a_pos.cmp(b_pos).then_with(|| a_path.cmp(b_path))
    });

    Ok(positioned_files
        .into_iter()
        .map(|(_, _, cli_id)| cli_id)
        .collect())
}

/// Internal helper for parsing sources with disambiguation prompts.
pub fn parse_sources_with_disambiguation(
    ctx: &mut Context,
    id_map: &IdMap,
    source: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<Vec<CliId>> {
    // Check if it's a list (contains ',')
    if source.contains(',') {
        return parse_list_with_disambiguation(ctx, id_map, source, out);
    }

    // Check if it's a valid range (e.g., "g0-h2" where both sides are uncommitted files).
    // If the string contains '-' but isn't a valid range (e.g., a filename like "my-file.rs"
    // or a branch name like "feature-auth"), fall through to single-entity parsing.
    if source.contains('-')
        && let Some(range_result) = try_parse_range(ctx, id_map, source)?
    {
        return Ok(range_result);
    }

    // Single source (including strings with dashes that aren't valid ranges)
    let source_result = id_map.parse_using_context(source, ctx)?;
    if source_result.is_empty() {
        return Err(anyhow::anyhow!(
            "Source '{source}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
        ));
    }

    if source_result.len() > 1 {
        // Ambiguous - prompt the user to disambiguate
        let selected = prompt_for_disambiguation(source, source_result, "the source", out)?;
        return Ok(vec![selected]);
    }

    Ok(vec![source_result[0].clone()])
}

/// Internal helper for parsing comma-separated lists with disambiguation support.
fn parse_list_with_disambiguation(
    ctx: &mut Context,
    id_map: &IdMap,
    source: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<Vec<CliId>> {
    let parts: Vec<&str> = source.split(',').collect();
    let mut result = Vec::new();

    for part in parts {
        let part = part.trim();

        // Skip empty parts (e.g., from input like "," or "a,,b")
        if part.is_empty() {
            continue;
        }

        let matches = id_map.parse_using_context(part, ctx)?;
        if matches.is_empty() {
            return Err(anyhow::anyhow!(
                "Item '{part}' in list not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
            ));
        }

        if matches.len() == 1 {
            result.push(matches[0].clone());
        } else {
            // Ambiguous - prompt the user to disambiguate
            let selected =
                prompt_for_disambiguation(part, matches, &format!("item '{part}' in list"), out)?;
            result.push(selected);
        }
    }

    // If all parts were empty, return an error
    if result.is_empty() {
        return Err(anyhow::anyhow!(
            "Source list '{source}' contains no valid items"
        ));
    }

    Ok(result)
}

fn parse_list(ctx: &mut Context, id_map: &IdMap, source: &str) -> anyhow::Result<Vec<CliId>> {
    let parts: Vec<&str> = source.split(',').collect();
    let mut result = Vec::new();

    for part in parts {
        let part = part.trim();

        // Skip empty parts (e.g., from input like "," or "a,,b")
        if part.is_empty() {
            continue;
        }

        let matches = id_map.parse_using_context(part, ctx)?;
        if matches.len() != 1 {
            if matches.is_empty() {
                return Err(anyhow::anyhow!(
                    "Item '{part}' in list not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
                ));
            } else {
                return Err(anyhow::anyhow!(
                    "Item '{part}' in list is ambiguous. Try using more characters to disambiguate."
                ));
            }
        }
        result.push(matches[0].clone());
    }

    // If all parts were empty, return an error
    if result.is_empty() {
        return Err(anyhow::anyhow!(
            "Source list '{source}' contains no valid items"
        ));
    }

    Ok(result)
}

/// Prompts the user to disambiguate between multiple CLI ID matches.
///
/// # Arguments
/// * `entity_str` - The original string the user typed
/// * `matches` - The possible matches (must not be empty)
/// * `context` - Description of what we're resolving (e.g., "source", "target")
/// * `out` - Output channel to check if environment is interactive
///
/// # Returns
/// The selected CliId from the user's choice
///
/// # Errors
/// Returns an error if the environment is non-interactive or if the user cancels the selection
pub fn prompt_for_disambiguation(
    entity_str: &str,
    matches: Vec<CliId>,
    context: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<CliId> {
    use cli_prompts::{DisplayPrompt, prompts::Selection};

    // Defensive check
    if matches.is_empty() {
        return Err(anyhow::anyhow!(
            "Internal error: prompt_for_disambiguation called with empty matches"
        ));
    }

    if !out.can_prompt() {
        // In non-interactive mode, show all options and error
        let options_str = matches
            .iter()
            .enumerate()
            .map(|(i, id)| {
                format!(
                    "  {}. {} ({})",
                    i + 1,
                    id.to_short_string(),
                    id.kind_for_humans()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        return Err(anyhow::anyhow!(
            "'{entity_str}' is ambiguous for {context}. Cannot prompt in non-interactive mode. Matches:\n{options_str}"
        ));
    }

    // Build options with clear descriptions
    let options: Vec<String> = matches
        .iter()
        .map(|id| {
            let short_id = id.to_short_string();
            let kind = id.kind_for_humans();

            // Add additional context based on the type
            match id {
                CliId::Commit { commit_id, .. } => {
                    format!(
                        "{} - {} (commit {})",
                        short_id,
                        kind,
                        &commit_id.to_string()[..7]
                    )
                }
                CliId::Branch { name, .. } => {
                    format!("{short_id} - {kind} (branch '{name}')")
                }
                CliId::CommittedFile {
                    path, commit_id, ..
                } => {
                    format!(
                        "{} - {} (file '{}' in commit {})",
                        short_id,
                        kind,
                        path,
                        &commit_id.to_string()[..7]
                    )
                }
                CliId::Uncommitted(uncommitted) => {
                    if uncommitted.is_entire_file {
                        let first_hunk = uncommitted.hunk_assignments.first();
                        format!("{} - {} (file '{}')", short_id, kind, first_hunk.path)
                    } else {
                        format!("{short_id} - {kind} (hunk)")
                    }
                }
                _ => format!("{short_id} - {kind}"),
            }
        })
        .collect();

    let prompt = Selection::new(
        &format!("'{entity_str}' matches multiple objects for {context}. Which one did you mean?"),
        options.iter().cloned(),
    );

    let selection_str = prompt
        .display()
        .map_err(|e| anyhow::anyhow!("Selection aborted: {e:?}"))?;

    // Find the index of the selected option - more robust than parsing IDs from the string
    let selection_index = options
        .iter()
        .position(|opt| opt == &selection_str)
        .ok_or_else(|| {
            anyhow::anyhow!("Internal error: selected option not found in options list")
        })?;

    // Use the index to get the corresponding CliId
    matches.into_iter().nth(selection_index).ok_or_else(|| {
        anyhow::anyhow!("Internal error: selection index {selection_index} out of bounds")
    })
}
