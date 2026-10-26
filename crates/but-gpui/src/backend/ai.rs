//! Writing commit messages and pull request descriptions with the AI provider set up in
//! Settings, streamed so the text appears as it's written. The prompts are Lite's, so the
//! apps write alike.

use anyhow::{Context as _, Result};
use but_llm::{ChatMessage, LLMProvider};

pub const COMMIT_SYSTEM: &str = "You write Git commit messages. Return only the commit message, without Markdown or commentary.";

const COMMIT_INSTRUCTIONS: &str = "Write a commit message for these changes.
Only respond with the commit message, without notes or Markdown.
Explain what changed and why, focusing on the most important changes.
Use present tense and a semantic commit prefix.
Keep the title under 50 characters and hard-wrap body lines at 72 characters.
Do not use emoji or start lines with the hash symbol.";

pub const PR_SYSTEM: &str = "You write pull requests.
Return the title on the first line, then a blank line, then the description body as Markdown.
The title is plain text: no leading \"#\", no quotes, no prefix.
Never repeat the title as a heading or first line of the body — it is shown above the body already.
Return nothing else: no commentary, no surrounding code fence.";

pub const RESOLVE_SYSTEM: &str = "You resolve merge conflicts in code. Return only the lines that \
replace the conflict, exactly as they belong in the file: no Markdown, no code fence, no \
commentary.";

/// How much diff or history goes to the model.
const CONTEXT_LIMIT: usize = 5_000;

fn clipped(text: &str) -> &str {
    match text.char_indices().nth(CONTEXT_LIMIT) {
        Some((at, _)) => text.get(..at).unwrap_or(text),
        None => text,
    }
}

/// What to ask for a commit message of `diff`, with what the author already wrote as notes.
pub fn commit_prompt(notes: &str, diff: &str) -> String {
    let notes = match notes.trim() {
        "" => String::new(),
        notes => format!("\n\nThe author's notes on it:\n```\n{notes}\n```"),
    };
    format!(
        "{COMMIT_INSTRUCTIONS}{notes}\n\nSelected changes:\n```diff\n{}\n```",
        clipped(diff)
    )
}

/// What to ask for a pull request's title and description, from its commit messages
/// (oldest first) and what's written so far.
pub fn pr_prompt(title: &str, body: &str, messages: &[String]) -> String {
    let messages = messages.join("\n\n---\n\n");
    let mut sections = vec![
        "Write the title and description for this pull request.".to_owned(),
        "List the most important changes. Use bullet points. Be concise.".to_owned(),
    ];
    if !title.trim().is_empty() {
        sections.push(format!("Working title:\n```\n{}\n```", title.trim()));
    }
    if !body.trim().is_empty() {
        sections.push(format!(
            "Description so far, to build on:\n```\n{}\n```",
            body.trim()
        ));
    }
    sections.push(format!(
        "Commit messages:\n```\n{}\n```",
        clipped(&messages)
    ));
    sections.join("\n\n")
}

/// What to ask for a merge of one conflict in `path` between two sides, each with its name
/// in a sentence, like "main" and "the commit \"Round totals\"", given the lines around them
/// and, when known, the lines before either changed them.
pub fn resolve_prompt(
    path: &str,
    names: [&str; 2],
    sides: [&str; 2],
    ancestor: Option<&str>,
    around: [&str; 2],
) -> String {
    let block = |label: String, text: &str| format!("{label}\n```\n{}\n```", clipped(text));
    let mut sections = vec![
        format!(
            "In {path}, {} and {} both changed the same lines. Merge them, keeping what each \
             meant to do.",
            names[1], names[0]
        ),
        block("The lines before:".into(), around[0]),
        block(format!("{} has:", names[0]), sides[0]),
        block(format!("{} has:", names[1]), sides[1]),
    ];
    if let Some(ancestor) = ancestor {
        sections.push(block("Before either changed them:".into(), ancestor));
    }
    sections.push(block("The lines after:".into(), around[1]));
    sections.push("Return only the merged lines.".into());
    sections.join("\n\n")
}

/// Ask the configured provider, calling `on_text` with each piece of the answer as it
/// arrives. Returns the whole answer.
fn stream(
    system: &str,
    prompt: String,
    on_text: impl Fn(&str) + Send + Sync + 'static,
) -> Result<String> {
    let config = gix::config(None, &gix::open::Options::default())?;
    let provider =
        LLMProvider::from_git_config(&config).with_context(
            || match super::accounts::gitbutler_user() {
                Some(_) => "AI isn't set up. Choose a provider in Settings → AI.",
                None => {
                    "AI isn't set up. Sign in to GitButler to use its key, or choose a provider \
                 and key in Settings → AI."
                }
            },
        )?;
    let model = provider.model_or_default();
    provider
        .stream_response(system, vec![ChatMessage::User(prompt)], &model, on_text)?
        .map(|answer| without_thinking(&answer).to_owned())
        .filter(|answer| !answer.trim().is_empty())
        .context("The AI provider answered with nothing")
}

/// [`stream`] off the UI thread, calling `show` with the answer so far as it arrives.
pub async fn write(
    system: &'static str,
    prompt: String,
    mut show: impl FnMut(&str),
) -> Result<String> {
    let (sender, mut tokens) = futures::channel::mpsc::unbounded::<String>();
    let answer = super::run(move || {
        stream(system, prompt, move |token| {
            sender.unbounded_send(token.to_owned()).ok();
        })
    });
    // Ends when `stream` returns and drops the sender.
    let pump = async {
        let mut text = String::new();
        while let Some(token) = futures::StreamExt::next(&mut tokens).await {
            text.push_str(&token);
            show(without_thinking(&text));
        }
    };
    futures::join!(answer, pump).0
}

/// Split a generated pull request into its title and body, dropping a fence around the
/// whole answer and a first body line that only repeats the title. Safe on a partial
/// answer, as it streams.
pub fn split_title_body(text: &str) -> (String, String) {
    let text = strip_fence(text);
    let clean = |line: &str| {
        line.trim()
            .trim_start_matches('#')
            .trim()
            .trim_matches(|c| matches!(c, '"' | '\'' | '`'))
            .trim()
            .to_owned()
    };
    let Some((title, rest)) = text.split_once('\n') else {
        return (clean(text), String::new());
    };
    let title = clean(title);
    let rest = rest.trim();
    let (first, after) = rest.split_once('\n').unwrap_or((rest, ""));
    let body = if !title.is_empty() && clean(first).eq_ignore_ascii_case(&title) {
        after.trim()
    } else {
        rest
    };
    (title, body.to_owned())
}

/// `text` without the reasoning a reasoning model, as DeepSeek-R1 or Qwen3 run locally, can
/// put before its answer in `<think>` tags, and nothing while it's still reasoning.
fn without_thinking(text: &str) -> &str {
    match text.trim_start().strip_prefix("<think>") {
        // Only blank lines go, as a suggested merge's first line keeps its indentation.
        Some(thinking) => (thinking.split_once("</think>"))
            .map_or("", |(_, answer)| answer.trim_start_matches(['\n', '\r'])),
        None => text,
    }
}

/// `text` without a code fence around it, also while it streams in. Only blank lines go
/// around it, as code, a suggested merge's above all, needs its first line's indentation.
pub fn strip_fence(text: &str) -> &str {
    fn lines(text: &str) -> &str {
        text.trim_start_matches(['\n', '\r']).trim_end()
    }
    let text = lines(text);
    let Some(after) = text.trim_start().strip_prefix("```") else {
        return text;
    };
    // Past the opener's language tag; a lone opener mid-stream goes too.
    let Some((_, inner)) = after.split_once('\n') else {
        return "";
    };
    match inner.trim_end().strip_suffix("```") {
        Some(inner) if !inner.contains("```") => lines(inner),
        Some(_) => text,
        None if inner.contains("```") => text,
        None => lines(inner),
    }
}

#[cfg(test)]
mod tests {
    use super::{split_title_body, strip_fence, without_thinking};

    #[test]
    fn a_reasoning_models_thinking_is_left_out() {
        let answer = "<think>\nThe diff adds pagination.\n</think>\n\nPaginate orders";
        assert_eq!(
            without_thinking(answer),
            "Paginate orders",
            "only the answer goes in"
        );
        assert_eq!(
            without_thinking("<think>\nThe diff"),
            "",
            "nothing shows while it reasons"
        );
        assert_eq!(
            without_thinking("<think>merge</think>\n\n    return a"),
            "    return a",
            "code after it keeps its first line's indentation"
        );
        assert_eq!(
            without_thinking("Mention <think> tags"),
            "Mention <think> tags",
            "an answer that only names the tag stays"
        );
    }

    #[test]
    fn a_suggested_merge_keeps_its_first_lines_indentation() {
        let code = "    return a\n}";
        assert_eq!(strip_fence(code), code, "unfenced, only blank lines go");
        assert_eq!(
            strip_fence("\n```python\n    return a\n}\n```\n"),
            code,
            "fenced, the fence and blank lines go, the indentation stays"
        );
    }

    #[test]
    fn splits_title_from_body_and_drops_an_echoed_heading() {
        let (title, body) =
            split_title_body("```markdown\n# Add search\n\n# Add search\n- Fast\n```");
        assert_eq!(title, "Add search", "fence and heading marks go");
        assert_eq!(body, "- Fast", "the body doesn't repeat the title");
        let (title, body) = split_title_body("Add search");
        assert_eq!(
            (title.as_str(), body.as_str()),
            ("Add search", ""),
            "a partial answer is all title"
        );
    }
}
