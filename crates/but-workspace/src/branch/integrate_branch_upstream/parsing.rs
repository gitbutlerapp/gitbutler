use std::collections::HashSet;

use anyhow::{Context as _, Result, bail};
use bstr::{BStr, ByteSlice};

use super::{
    IntegrationDivergenceDisplay, IntegrationDivergenceTargetRelation, InteractiveIntegrationStep,
};

/// Render integration steps into their canonical executable text form.
pub fn render_integration_steps_script(steps: &[InteractiveIntegrationStep]) -> String {
    let mut lines = steps.iter().map(render_step).collect::<Vec<_>>();
    lines.push(String::new());
    lines.join("\n")
}

/// Parse an edited integration script back into executable integration steps.
pub fn parse_integration_steps_script(
    script: impl AsRef<[u8]>,
    divergence: &IntegrationDivergenceDisplay,
) -> Result<Vec<InteractiveIntegrationStep>> {
    let script = BStr::new(script.as_ref());
    let allowed_commits = editable_candidate_commits(divergence);
    let mut steps = Vec::new();

    for (line_number, line) in script.lines().enumerate() {
        let line_number = line_number + 1;
        let line = line.to_str_lossy();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let (command_part, message_part) = split_message_clause(trimmed)
            .with_context(|| format!("line {line_number}: invalid message clause"))?;
        let tokens = command_part.split_whitespace().collect::<Vec<_>>();
        let Some(command) = tokens.first().copied() else {
            continue;
        };
        let arguments = tokens.get(1..).unwrap_or_default();

        let step = match command {
            "pick" => {
                if message_part.is_some() {
                    bail!("line {line_number}: pick does not accept a message clause");
                }
                if arguments.len() != 1 {
                    bail!("line {line_number}: pick requires exactly one commit");
                }
                let commit = arguments
                    .first()
                    .copied()
                    .expect("validated pick arity above");
                InteractiveIntegrationStep::Pick {
                    commit_id: resolve_commit(commit, &allowed_commits).map_err(|err| {
                        anyhow::anyhow!("line {line_number}: invalid pick commit: {err}")
                    })?,
                }
            }
            "merge" => {
                if message_part.is_some() {
                    bail!("line {line_number}: merge does not accept a message clause");
                }
                if arguments.len() != 1 {
                    bail!("line {line_number}: merge requires exactly one commit");
                }
                let commit = arguments
                    .first()
                    .copied()
                    .expect("validated merge arity above");
                InteractiveIntegrationStep::Merge {
                    commit_id: resolve_commit(commit, &allowed_commits).map_err(|err| {
                        anyhow::anyhow!("line {line_number}: invalid merge commit: {err}")
                    })?,
                }
            }
            "squash" => {
                if arguments.len() < 2 {
                    bail!("line {line_number}: squash requires at least two commits");
                }
                let commits = arguments
                    .iter()
                    .map(|commit| {
                        resolve_commit(commit, &allowed_commits).map_err(|err| {
                            anyhow::anyhow!(
                                "line {line_number}: invalid squash commit '{commit}': {err}"
                            )
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                InteractiveIntegrationStep::Squash {
                    commits,
                    message: message_part
                        .map(parse_message_clause)
                        .transpose()
                        .map_err(|err| {
                            anyhow::anyhow!("line {line_number}: invalid squash message: {err}")
                        })?,
                }
            }
            other => bail!("line {line_number}: unknown command '{other}'"),
        };
        steps.push(step);
    }

    Ok(steps)
}

fn render_step(step: &InteractiveIntegrationStep) -> String {
    match step {
        InteractiveIntegrationStep::Pick { commit_id } => format!("pick {}", short_id(*commit_id)),
        InteractiveIntegrationStep::Merge { commit_id } => {
            format!("merge {}", short_id(*commit_id))
        }
        InteractiveIntegrationStep::Squash { commits, message } => {
            let mut rendered = format!(
                "squash {}",
                commits
                    .iter()
                    .map(|commit| short_id(*commit))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            if let Some(message) = message {
                rendered.push_str(" | message=");
                rendered.push_str(&quote_message(message));
            }
            rendered
        }
    }
}

fn short_id(commit_id: gix::ObjectId) -> String {
    commit_id.to_hex_with_len(7).to_string()
}

fn editable_candidate_commits(divergence: &IntegrationDivergenceDisplay) -> HashSet<gix::ObjectId> {
    divergence
        .local_only
        .iter()
        .filter(|commit| {
            matches!(
                commit.target_relation,
                IntegrationDivergenceTargetRelation::NotIntegrated
            )
        })
        .map(|commit| commit.id)
        .chain(divergence.upstream_only.iter().map(|commit| commit.id))
        .collect()
}

fn resolve_commit(spec: &str, allowed_commits: &HashSet<gix::ObjectId>) -> Result<gix::ObjectId> {
    let matches = allowed_commits
        .iter()
        .copied()
        .filter(|commit_id| commit_id.to_string().starts_with(spec))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => bail!("commit '{spec}' is not part of the editable divergence"),
        [commit_id] => Ok(*commit_id),
        _ => bail!("commit prefix '{spec}' is ambiguous"),
    }
}

fn split_message_clause(line: &str) -> Result<(&str, Option<&str>)> {
    let Some((command_part, message_part)) = line.split_once('|') else {
        return Ok((line, None));
    };
    Ok((command_part.trim_end(), Some(message_part.trim_start())))
}

fn parse_message_clause(clause: &str) -> Result<String> {
    let value = clause
        .strip_prefix("message=")
        .context("expected `message=` after `|`")?;
    parse_quoted_string(value)
}

fn parse_quoted_string(input: &str) -> Result<String> {
    if !input.starts_with('"') {
        bail!("message must start with a double quote");
    }

    let mut output = String::new();
    let mut escaped = false;
    for (index, ch) in input.char_indices().skip(1) {
        if escaped {
            let resolved = match ch {
                '\\' => '\\',
                '"' => '"',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => bail!("unsupported escape sequence '\\{other}'"),
            };
            output.push(resolved);
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => {
                let trailing = input[index + ch.len_utf8()..].trim();
                if trailing.is_empty() {
                    return Ok(output);
                }
                bail!("unexpected trailing characters after quoted message");
            }
            other => output.push(other),
        }
    }

    if escaped {
        bail!("unterminated escape sequence in quoted message");
    }
    bail!("unterminated quoted message")
}

fn quote_message(message: &str) -> String {
    let mut out = String::from("\"");
    for ch in message.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch::IntegrationDivergenceCommit;
    use crate::ui::Author;

    fn oid(hex: &str) -> gix::ObjectId {
        gix::ObjectId::from_hex(hex.as_bytes()).expect("hex object id")
    }

    fn author() -> Author {
        Author {
            name: "Test Author".to_owned(),
            email: "author@example.com".to_owned(),
            gravatar_url: url::Url::parse("https://example.com/avatar.png")
                .expect("valid author url"),
        }
    }

    fn divergence() -> IntegrationDivergenceDisplay {
        IntegrationDivergenceDisplay {
            branch_ref_name: gix::refs::FullName::try_from("refs/heads/feature")
                .expect("valid local ref"),
            upstream_ref_name: gix::refs::FullName::try_from("refs/remotes/origin/feature")
                .expect("valid remote ref"),
            local_only: vec![
                IntegrationDivergenceCommit {
                    id: oid("1111111aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                    subject: "Integrated local".to_owned(),
                    change_id: None,
                    created_at: 0,
                    author: author(),
                    refs: vec!["feature".to_owned()],
                    target_relation: IntegrationDivergenceTargetRelation::HistoricallyIntegrated {
                        target_commit_id: oid("9999999aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                    },
                },
                IntegrationDivergenceCommit {
                    id: oid("2222222aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                    subject: "Editable local".to_owned(),
                    change_id: None,
                    created_at: 0,
                    author: author(),
                    refs: vec![],
                    target_relation: IntegrationDivergenceTargetRelation::NotIntegrated,
                },
            ],
            upstream_only: vec![
                IntegrationDivergenceCommit {
                    id: oid("3333333aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                    subject: "Upstream one".to_owned(),
                    change_id: None,
                    created_at: 0,
                    author: author(),
                    refs: vec!["origin/feature".to_owned()],
                    target_relation: IntegrationDivergenceTargetRelation::NotIntegrated,
                },
                IntegrationDivergenceCommit {
                    id: oid("4444444aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                    subject: "Upstream two".to_owned(),
                    change_id: None,
                    created_at: 0,
                    author: author(),
                    refs: vec![],
                    target_relation: IntegrationDivergenceTargetRelation::NotIntegrated,
                },
            ],
            merge_base: IntegrationDivergenceCommit {
                id: oid("5555555aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                subject: "Merge base".to_owned(),
                change_id: None,
                created_at: 0,
                author: author(),
                refs: vec![],
                target_relation: IntegrationDivergenceTargetRelation::NotIntegrated,
            },
        }
    }

    #[test]
    fn renders_then_parses_simple_pick_steps() {
        let divergence = divergence();
        let steps = vec![
            InteractiveIntegrationStep::Pick {
                commit_id: oid("3333333aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            },
            InteractiveIntegrationStep::Pick {
                commit_id: oid("2222222aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            },
        ];

        let script = render_integration_steps_script(&steps);
        let parsed = parse_integration_steps_script(script.as_bytes(), &divergence)
            .expect("script should round-trip");

        assert_eq!(
            rendered_steps(&parsed),
            rendered_steps(&steps),
            "simple picks should round-trip through the script format"
        );
    }

    #[test]
    fn parses_mixed_steps_with_inline_message() {
        let divergence = divergence();
        let parsed = parse_integration_steps_script(
            br#"
pick 3333333
merge 4444444
squash 2222222 3333333 4444444 | message="combined \"message\""
"#,
            &divergence,
        )
        .expect("valid mixed script");

        assert_eq!(
            rendered_steps(&parsed),
            vec![
                "pick 3333333aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                "merge 4444444aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                "squash 2222222aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3333333aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 4444444aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | message=\"combined \\\"message\\\"\"".to_owned(),
            ],
            "parser should handle pick, merge, squash, and inline quoted messages"
        );
    }

    #[test]
    fn ignores_blank_lines_and_comments() {
        let divergence = divergence();
        let parsed = parse_integration_steps_script(
            br#"
# comment

pick 3333333
  # another comment
merge 4444444
"#,
            &divergence,
        )
        .expect("comments should be ignored");

        assert_eq!(
            rendered_steps(&parsed),
            vec![
                "pick 3333333aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                "merge 4444444aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            ],
            "comment and blank lines should not affect parsing"
        );
    }

    #[test]
    fn accepts_full_hashes() {
        let divergence = divergence();
        let parsed = parse_integration_steps_script(
            br#"pick 3333333aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"#,
            &divergence,
        )
        .expect("full hash should resolve");

        assert_eq!(
            rendered_steps(&parsed),
            vec!["pick 3333333aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned()],
            "full hashes should be accepted"
        );
    }

    #[test]
    fn rejects_ambiguous_short_hashes() {
        let divergence = IntegrationDivergenceDisplay {
            upstream_only: vec![
                IntegrationDivergenceCommit {
                    id: oid("abcdef0aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                    subject: "A".to_owned(),
                    change_id: None,
                    created_at: 0,
                    author: author(),
                    refs: vec![],
                    target_relation: IntegrationDivergenceTargetRelation::NotIntegrated,
                },
                IntegrationDivergenceCommit {
                    id: oid("abcdef1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                    subject: "B".to_owned(),
                    change_id: None,
                    created_at: 0,
                    author: author(),
                    refs: vec![],
                    target_relation: IntegrationDivergenceTargetRelation::NotIntegrated,
                },
            ],
            ..divergence()
        };

        let err = parse_integration_steps_script(br#"pick abcdef"#, &divergence)
            .expect_err("ambiguous prefixes should be rejected");

        assert_eq!(
            err.to_string(),
            "line 1: invalid pick commit: commit prefix 'abcdef' is ambiguous",
            "ambiguous short hashes should point at the offending line"
        );
    }

    #[test]
    fn rejects_unknown_commit() {
        let err = parse_integration_steps_script(br#"pick deadbee"#, &divergence())
            .expect_err("unknown commit should fail");

        assert_eq!(
            err.to_string(),
            "line 1: invalid pick commit: commit 'deadbee' is not part of the editable divergence",
            "unknown commits should be rejected against the editable divergence set"
        );
    }

    #[test]
    fn rejects_integrated_local_commit() {
        let err = parse_integration_steps_script(br#"pick 1111111"#, &divergence())
            .expect_err("integrated locals should not be editable");

        assert_eq!(
            err.to_string(),
            "line 1: invalid pick commit: commit '1111111' is not part of the editable divergence",
            "historically integrated local commits should stay out of the editable set"
        );
    }

    #[test]
    fn rejects_malformed_command() {
        let err = parse_integration_steps_script(br#"drop 3333333"#, &divergence())
            .expect_err("unknown commands should fail");

        assert_eq!(
            err.to_string(),
            "line 1: unknown command 'drop'",
            "unexpected commands should be rejected explicitly"
        );
    }

    #[test]
    fn rejects_wrong_arity_for_pick_and_merge() {
        let pick_err = parse_integration_steps_script(br#"pick 3333333 4444444"#, &divergence())
            .expect_err("pick arity should be checked");
        let merge_err = parse_integration_steps_script(br#"merge"#, &divergence())
            .expect_err("merge arity should be checked");

        assert_eq!(
            pick_err.to_string(),
            "line 1: pick requires exactly one commit",
            "pick should require exactly one argument"
        );
        assert_eq!(
            merge_err.to_string(),
            "line 1: merge requires exactly one commit",
            "merge should require exactly one argument"
        );
    }

    #[test]
    fn rejects_squash_with_too_few_commits() {
        let err = parse_integration_steps_script(br#"squash 3333333"#, &divergence())
            .expect_err("squash needs at least two commits");

        assert_eq!(
            err.to_string(),
            "line 1: squash requires at least two commits",
            "squash should require two or more commits"
        );
    }

    fn rendered_steps(steps: &[InteractiveIntegrationStep]) -> Vec<String> {
        steps.iter().map(ToString::to_string).collect()
    }
}
