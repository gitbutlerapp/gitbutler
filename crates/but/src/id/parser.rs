use bstr::BStr;
use but_ctx::Context;

use crate::{CliId, IdMap, id::SourceScope};

#[derive(Debug)]
pub(crate) struct IdResolutionError(String);

impl IdResolutionError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for IdResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for IdResolutionError {}

fn parse_scoped(
    ctx: &mut Context,
    id_map: &IdMap,
    part: &str,
    scope: SourceScope,
) -> anyhow::Result<Vec<CliId>> {
    match scope {
        SourceScope::Any => id_map.parse_using_context(part, ctx),
        SourceScope::UncommittedOnly => resolve_uncommitted_part(ctx, id_map, part),
    }
}

/// Resolve one selector that must name uncommitted changes: the uncommitted
/// namespace first, then a full-namespace fallback that keeps any uncommitted
/// interpretations (container selectors the scoped parser does not model) and
/// turns everything else into a targeted error naming what the selector is.
///
/// This is the single home of that policy, used by callers that resolve a
/// selector under [`SourceScope::UncommittedOnly`] — currently `but absorb`.
pub(crate) fn resolve_uncommitted_part(
    ctx: &mut Context,
    id_map: &IdMap,
    part: &str,
) -> anyhow::Result<Vec<CliId>> {
    let scoped = id_map.parse_uncommitted_using_context(part, ctx)?;
    if !scoped.is_empty() {
        return Ok(scoped);
    }
    let full = id_map.parse_using_context(part, ctx)?;
    let uncommitted: Vec<CliId> = full
        .iter()
        .filter(|id| matches!(id, CliId::UncommittedHunkOrFile(_)))
        .cloned()
        .collect();
    if !uncommitted.is_empty() {
        return Ok(uncommitted);
    }
    if let Some(other) = full.first() {
        return Err(IdResolutionError::new(format!(
            "'{}' is {} but must be an uncommitted file or hunk",
            part,
            other.kind_for_humans()
        ))
        .into());
    }
    Ok(vec![])
}

pub(crate) fn parse_sources(
    ctx: &mut Context,
    id_map: &IdMap,
    source: &str,
) -> anyhow::Result<Vec<CliId>> {
    parse_sources_scoped(ctx, id_map, source, SourceScope::Any)
}

fn parse_sources_scoped(
    ctx: &mut Context,
    id_map: &IdMap,
    source: &str,
    scope: SourceScope,
) -> anyhow::Result<Vec<CliId>> {
    // Check if it's a list (contains ',')
    if source.contains(',') {
        return parse_list(ctx, id_map, source, scope);
    }

    // Check if it's a valid range (e.g., "g0-h2" where both sides are uncommitted files).
    // If the string contains '-' but isn't a valid range (e.g., a filename like "my-file.rs"
    // or a branch name like "feature-auth"), fall through to single-entity parsing.
    if source.contains('-')
        && let Some(range_result) = try_parse_range(ctx, id_map, source, scope)?
    {
        return Ok(range_result);
    }

    // Single source (including strings with dashes that aren't valid ranges)
    let source_result = parse_scoped(ctx, id_map, source, scope)?;
    if source_result.len() != 1 {
        if source_result.is_empty() {
            return Err(IdResolutionError::new(format!(
                "Source '{source}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
            ))
            .into());
        } else {
            let matches: Vec<String> = source_result
                .iter()
                .map(|id| format!("{} ({})", id.to_short_string(), id.kind_for_humans()))
                .collect();
            return Err(IdResolutionError::new(format!(
                "Source '{}' is ambiguous. Matches: {}. Try using more characters, a longer SHA, or the full branch name to disambiguate.",
                source,
                matches.join(", ")
            ))
            .into());
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
    scope: SourceScope,
) -> anyhow::Result<Option<Vec<CliId>>> {
    let parts: Vec<&str> = source.split('-').collect();
    if parts.len() != 2 {
        return Ok(None);
    }

    // If either half fails to parse (e.g., single character "a" in "a-file.txt"),
    // this isn't a range — fall through to single-entity parsing. Endpoints
    // honor the caller's scope: a range copied from `but diff` must keep
    // resolving even when a later commit's change ID shadows an endpoint in
    // the full namespace.
    let Ok(start_matches) = parse_scoped(ctx, id_map, parts[0], scope) else {
        return Ok(None);
    };
    let Ok(end_matches) = parse_scoped(ctx, id_map, parts[1], scope) else {
        return Ok(None);
    };

    // Both sides must resolve to exactly one Uncommitted entity
    if start_matches.len() != 1 || end_matches.len() != 1 {
        return Ok(None);
    }
    if !matches!(&start_matches[0], CliId::UncommittedHunkOrFile(_))
        || !matches!(&end_matches[0], CliId::UncommittedHunkOrFile(_))
    {
        return Ok(None);
    }

    // Valid range — resolve positions in display order
    let all_files = get_all_files_in_display_order(id_map)?;
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

fn get_all_files_in_display_order(id_map: &IdMap) -> anyhow::Result<Vec<CliId>> {
    let mut files: Vec<(&BStr, CliId)> = id_map
        .uncommitted_files
        .values()
        .map(|uncommitted_file| (uncommitted_file.path(), uncommitted_file.to_id()))
        .collect();
    files.sort_by_key(|(a_path, _)| *a_path);

    Ok(files.into_iter().map(|(_, cli_id)| cli_id).collect())
}

fn parse_list(
    ctx: &mut Context,
    id_map: &IdMap,
    source: &str,
    scope: SourceScope,
) -> anyhow::Result<Vec<CliId>> {
    let parts: Vec<&str> = source.split(',').collect();
    let mut result = Vec::new();

    for part in parts {
        let part = part.trim();

        // Skip empty parts (e.g., from input like "," or "a,,b")
        if part.is_empty() {
            continue;
        }

        let matches = parse_scoped(ctx, id_map, part, scope)?;
        if matches.len() != 1 {
            if matches.is_empty() {
                return Err(IdResolutionError::new(format!(
                    "Item '{part}' in list not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
                ))
                .into());
            } else {
                return Err(IdResolutionError::new(format!(
                    "Item '{part}' in list is ambiguous. Try using more characters to disambiguate."
                ))
                .into());
            }
        }
        result.push(matches[0].clone());
    }

    // If all parts were empty, return an error
    if result.is_empty() {
        return Err(IdResolutionError::new(format!(
            "Source list '{source}' contains no valid items"
        ))
        .into());
    }

    Ok(result)
}
