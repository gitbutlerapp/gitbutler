//! Typed intent/declaration storage and dependency queries.

use std::collections::HashSet;

use rusqlite::{Connection, Transaction, params};

use super::{DependencyHint, PrepareSql, SurfaceDeclaration};
use crate::payloads::SurfacePayload;

/// Insert an intent or declaration row and its related lists.
pub(crate) fn insert_surface_declaration(
    tx: &Transaction<'_>,
    created_at_ms: i64,
    agent_id: &str,
    kind: &str,
    payload: &SurfacePayload,
) -> anyhow::Result<i64> {
    tx.execute(
        "INSERT INTO surface_declarations(created_at_ms, agent_id, kind, scope)
         VALUES (?1, ?2, ?3, ?4)",
        params![created_at_ms, agent_id, kind, payload.scope],
    )?;
    let declaration_id = tx.last_insert_rowid();
    for (ord, tag) in payload.tags.iter().enumerate() {
        tx.execute(
            "INSERT INTO surface_tags(declaration_id, ord, tag) VALUES (?1, ?2, ?3)",
            params![declaration_id, ord as i64, tag],
        )?;
    }
    for (ord, token) in payload.surface.iter().enumerate() {
        tx.execute(
            "INSERT INTO surface_tokens(declaration_id, ord, token) VALUES (?1, ?2, ?3)",
            params![declaration_id, ord as i64, token],
        )?;
    }
    for (ord, path) in payload.paths.iter().enumerate() {
        tx.execute(
            "INSERT INTO surface_paths(declaration_id, ord, path) VALUES (?1, ?2, ?3)",
            params![declaration_id, ord as i64, path],
        )?;
    }
    Ok(declaration_id)
}

/// Load all typed surface declarations.
pub(crate) fn load_surface_declarations(
    conn: &Connection,
) -> anyhow::Result<Vec<SurfaceDeclaration>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at_ms, agent_id, kind, scope
         FROM surface_declarations
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SurfaceDeclaration {
            id: row.get(0)?,
            created_at_ms: row.get(1)?,
            agent_id: row.get(2)?,
            kind: row.get(3)?,
            scope: row.get(4)?,
            tags: Vec::new(),
            surface: Vec::new(),
            paths: Vec::new(),
        })
    })?;
    let mut declarations = Vec::new();
    for row in rows {
        let mut declaration = row?;
        hydrate_surface_lists(conn, &mut declaration)?;
        declarations.push(declaration);
    }
    Ok(declarations)
}

/// Compute dependency hints scoped to the requested paths.
pub(crate) fn dependency_hints_for_paths(
    conn: &Connection,
    agent_id: &str,
    requested_paths: &[String],
) -> anyhow::Result<Vec<DependencyHint>> {
    if requested_paths.is_empty() {
        return Ok(Vec::new());
    }

    let all_declarations = load_surface_declarations(conn)?;
    let mut requester_intents = Vec::new();
    let mut declarations = Vec::new();
    for declaration in all_declarations {
        if declaration.kind == "intent" && declaration.agent_id == agent_id {
            let has_relevant_path = declaration.paths.is_empty()
                || declaration.paths.iter().any(|intent_path| {
                    requested_paths
                        .iter()
                        .any(|req| paths_overlap(intent_path, req))
                });
            if has_relevant_path {
                requester_intents.push(declaration);
            }
            continue;
        }

        if declaration.kind == "declaration" && declaration.agent_id != agent_id {
            declarations.push(declaration);
        }
    }
    if requester_intents.is_empty() {
        return Ok(Vec::new());
    }

    let mut hints = Vec::new();
    let mut seen_provider_scope = HashSet::<(String, String)>::new();
    for declaration in declarations {
        if !declaration.tags.iter().any(|tag| tag_has_api_segment(tag)) {
            continue;
        }

        for intent in &requester_intents {
            if !intent.scope.is_empty()
                && !declaration.scope.is_empty()
                && intent.scope != declaration.scope
            {
                continue;
            }

            let overlap_tokens: Vec<String> = declaration
                .surface
                .iter()
                .filter(|token| intent.surface.iter().any(|own| own == *token))
                .cloned()
                .collect();
            if overlap_tokens.is_empty() {
                continue;
            }

            let overlap_paths =
                scoped_overlap_paths(&intent.paths, &declaration.paths, requested_paths);
            let path_match = if intent.paths.is_empty() && declaration.paths.is_empty() {
                true
            } else {
                !overlap_paths.is_empty()
            };
            if !path_match {
                continue;
            }

            if !seen_provider_scope
                .insert((declaration.agent_id.clone(), declaration.scope.clone()))
            {
                continue;
            }

            let why = if overlap_paths.is_empty() {
                format!(
                    "intent/declaration overlap on token(s): {} within scope {}",
                    overlap_tokens.join(", "),
                    declaration.scope
                )
            } else {
                format!(
                    "intent/declaration overlap on token(s): {} for path(s): {}",
                    overlap_tokens.join(", "),
                    overlap_paths.join(", ")
                )
            };
            hints.push(DependencyHint {
                kind: "dependency_hint",
                provider_agent_id: declaration.agent_id.clone(),
                scope: declaration.scope.clone(),
                tags: declaration.tags.clone(),
                overlap_tokens,
                overlap_paths,
                why,
            });
        }
    }
    Ok(hints)
}

/// Check whether two repo-relative paths overlap literally.
pub(crate) fn paths_overlap(lhs: &str, rhs: &str) -> bool {
    lhs == rhs || lhs.starts_with(&format!("{rhs}/")) || rhs.starts_with(&format!("{lhs}/"))
}

/// Return path overlap between scoped intent/declaration data and requested paths.
fn scoped_overlap_paths(
    intent_paths: &[String],
    declaration_paths: &[String],
    requested_paths: &[String],
) -> Vec<String> {
    if intent_paths.is_empty() && declaration_paths.is_empty() {
        return Vec::new();
    }

    let mut overlap = Vec::new();
    if !intent_paths.is_empty() && !declaration_paths.is_empty() {
        for intent_path in intent_paths {
            if declaration_paths
                .iter()
                .any(|decl_path| paths_overlap(intent_path, decl_path))
            {
                overlap.push(intent_path.clone());
            }
        }
        overlap.sort();
        overlap.dedup();
        return overlap;
    }

    let scoped = if intent_paths.is_empty() {
        declaration_paths
    } else {
        intent_paths
    };
    for path in scoped {
        if requested_paths
            .iter()
            .any(|requested| paths_overlap(path, requested))
        {
            overlap.push(path.clone());
        }
    }
    overlap.sort();
    overlap.dedup();
    overlap
}

/// Check if a tag contains an `api` segment.
fn tag_has_api_segment(tag: &str) -> bool {
    tag.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|segment| segment.eq_ignore_ascii_case("api"))
}

/// Load ordered list values for one declaration field.
fn load_surface_list(
    conn: &impl PrepareSql,
    table: &str,
    column: &str,
    declaration_id: i64,
) -> anyhow::Result<Vec<String>> {
    let sql = format!("SELECT {column} FROM {table} WHERE declaration_id = ?1 ORDER BY ord ASC");
    let mut stmt = conn.prepare_query(&sql)?;
    let rows = stmt.query_map(params![declaration_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Hydrate tag/token/path lists for one declaration.
fn hydrate_surface_lists(
    conn: &Connection,
    declaration: &mut SurfaceDeclaration,
) -> anyhow::Result<()> {
    declaration.tags = load_surface_list(conn, "surface_tags", "tag", declaration.id)?;
    declaration.surface = load_surface_list(conn, "surface_tokens", "token", declaration.id)?;
    declaration.paths = load_surface_list(conn, "surface_paths", "path", declaration.id)?;
    Ok(())
}
