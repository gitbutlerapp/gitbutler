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
                .map(|id| format!("{} ({})", id.short_string(), id.kind_for_humans()))
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
