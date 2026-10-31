use but_ctx::Context;

use crate::{CliId, CliResult, IdMap, bad_input};

/// Resolve one selector that must name uncommitted changes: the uncommitted
/// namespace first, then a full-namespace fallback that keeps any uncommitted
/// interpretations (container selectors the scoped parser does not model) and
/// turns everything else into a targeted error naming what the selector is.
///
/// This is the single home of that policy, currently used by `but absorb`.
pub(crate) fn resolve_uncommitted_part(
    ctx: &mut Context,
    id_map: &IdMap,
    part: &str,
) -> CliResult<Vec<CliId>> {
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
        return Err(bad_input(format!(
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
) -> CliResult<Vec<CliId>> {
    // Check if it's a list (contains ',')
    if source.contains(',') {
        return parse_list(ctx, id_map, source);
    }

    let source_result = id_map.parse_using_context(source, ctx)?;
    if source_result.len() != 1 {
        if source_result.is_empty() {
            return Err(bad_input(format!(
                "Source '{source}' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
            ))
            .into());
        } else {
            let matches: Vec<String> = source_result
                .iter()
                .map(|id| format!("{} ({})", id.short_string(), id.kind_for_humans()))
                .collect();
            return Err(bad_input(format!(
                "Source '{}' is ambiguous. Matches: {}. Try using more characters, a longer SHA, or the full branch name to disambiguate.",
                source,
                matches.join(", ")
            ))
            .into());
        }
    }
    Ok(vec![source_result[0].clone()])
}

fn parse_list(ctx: &mut Context, id_map: &IdMap, source: &str) -> CliResult<Vec<CliId>> {
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
                return Err(bad_input(format!(
                    "Item '{part}' in list not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
                ))
                .into());
            } else {
                return Err(bad_input(format!(
                    "Item '{part}' in list is ambiguous. Try using more characters to disambiguate."
                ))
                .into());
            }
        }
        result.push(matches[0].clone());
    }

    // If all parts were empty, return an error
    if result.is_empty() {
        return Err(bad_input(format!("Source list '{source}' contains no valid items")).into());
    }

    Ok(result)
}
