//! The prompt and structured response contract for AI conflict resolution.
//!
//! The model only ever returns the merged replacement text for each conflict
//! block — never whole files — and the application splices those hunks back
//! into the merged blob deterministically.
//!
//! The response shape is deserialized by the provider's structured-output
//! support. The OpenAI path enforces the schema with strict mode, but other
//! providers only embed the schema in the prompt, so the system prompt also
//! spells out the exact JSON contract — without it, models drift to their own
//! field names and deserialization fails.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::context::{ConflictHunk, ResolutionRequest};

/// The system message sent along with every conflict-resolution request.
pub const SYSTEM_PROMPT: &str = "\
You are an expert Git conflict resolver. Analyze conflicts that occurred while a commit was rebased and produce correct, clean resolutions.

You will receive:
- The message of the conflicted commit (its intent) and the title of the new base it was rebased onto
- For each conflicted file, the conflicts as ours/base/theirs sections with surrounding context
- \"Ours\" is the state of the new base the commit is rebased onto; \"theirs\" is this commit's own version; \"base\" is their common ancestor

Your job:
1. Understand the INTENT behind each side's changes
2. Resolve each conflict by producing the correct merged content for each conflict hunk
3. Explain your reasoning per file — terse but specific enough to verify the decision
4. Produce a brief markdown summary orienting the user to the conflict and resolution

Resolution guidelines:
- Make MINIMAL changes — do not refactor, reformat, or alter code outside conflicted regions
- When both sides add complementary code (e.g., different imports), combine them
- When both sides modify the same code differently, use the commit message to decide what this commit intends
- When one side deletes code the other modifies, check whether the content was relocated rather than simply removed — accept the deletion only when it was intentional
- When conflicts involve dependency manifests or lock files, ensure version constraints and entries remain consistent across the resolved file
- Preserve correctness: imports, types, formatting must remain valid
- When in doubt, prefer backward compatibility

Respond ONLY with a single JSON object in exactly this shape:
{
  \"summary\": \"### Conflicting changes\\n<1-2 sentences: what each side did and where they collided>\\n\\n### Resolution\\n<1 sentence: how you resolved it; bold any trade-off where a side's change was dropped>\",
  \"resolutions\": [
    {
      \"path\": \"relative/file/path.py\",
      \"hunks\": [
        { \"resolvedContent\": \"merged content that replaces conflict 1\" },
        { \"resolvedContent\": \"merged content that replaces conflict 2\" }
      ],
      \"reasoning\": \"What each side changed in this file, what you kept, and what you dropped or overrode.\"
    }
  ]
}

Field rules:
- Return a data object in this shape — never the JSON schema itself.
- \"resolutions\" is REQUIRED: exactly one entry per conflicted file from the input, with \"path\" copied exactly as given.
- \"hunks\" is an ordered array with one entry per conflict, matching the \"Conflict 1 of N\", \"Conflict 2 of N\" order from the input. Each \"resolvedContent\" is ONLY the merged content that replaces that specific conflict block — no surrounding non-conflicted code and no conflict markers. To accept one side entirely, return that side's content verbatim. For an intentional deletion, use an empty string.
- \"reasoning\" is required per file: terse, direct prose — enough detail to verify the decision, typically 1-4 sentences.
- \"summary\" may be null; when present it is a markdown banner with exactly the two \"###\" headings shown above.";

/// The structured response produced by the model.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionResponse {
    #[schemars(
        description = "A brief markdown summary with exactly two '###' headings: 'Conflicting changes' (1-2 sentences on what each side did and where they collided) then 'Resolution' (1 sentence on how it was resolved; bold any trade-off where a side's change was dropped). Write natural prose a developer would say to a teammate; per-file detail belongs in the per-file reasoning, not here."
    )]
    /// A short model-authored markdown summary of the conflict and its resolution.
    #[serde(default)]
    pub summary: Option<String>,
    /// One entry per conflicted file that was provided in the request.
    pub resolutions: Vec<FileResolution>,
}

/// The model's resolution for a single conflicted file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileResolution {
    #[schemars(description = "The file path, exactly as given in the input.")]
    /// The repo-relative path of the resolved file, matching the request.
    pub path: String,
    #[schemars(
        description = "An ordered array with one entry per conflict in the file, matching the 'Conflict 1 of N', 'Conflict 2 of N' order from the input."
    )]
    /// The per-conflict resolutions, in input order.
    pub hunks: Vec<HunkContent>,
    #[schemars(
        description = "Terse, direct prose — enough detail to verify the decision, not a wall of text. State what each side did in this file, what you kept, and any trade-off. Typically 1-4 sentences."
    )]
    /// The model's per-file explanation of its decision.
    pub reasoning: String,
}

/// The replacement content for a single conflict block.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HunkContent {
    #[schemars(
        description = "ONLY the merged content that replaces this specific conflict block (the region between <<<<<<< and >>>>>>>). Do NOT include surrounding non-conflicted code — the application splices each resolution into the original file automatically. To accept one side entirely, return that side's content verbatim. For an intentional deletion, use an empty string."
    )]
    /// The merged text that replaces the conflict block, without any markers.
    pub resolved_content: String,
}

/// Render the user message for `request`, one `Conflict N of M` section per hunk.
pub fn render_user_message(request: &ResolutionRequest) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "Conflicts occurred while the commit below was rebased onto a new base{}.\n",
        request
            .parent_message
            .as_deref()
            .map(|title| format!(" (\"{title}\")"))
            .unwrap_or_default()
    ));
    out.push_str("\"Ours\" is the new base's version, \"theirs\" is the commit's own version.\n\n");
    out.push_str("## Commit message of the conflicted commit\n");
    out.push_str(&fenced_block(&request.commit_message, ""));
    out.push('\n');

    for file in &request.files {
        out.push_str(&format!("## File: {}\n\n", sanitize_path(&file.path)));
        let lang = language_tag(&file.path);
        let total = file.hunks.len();
        for (index, hunk) in file.hunks.iter().enumerate() {
            out.push_str(&format!("### Conflict {} of {total}\n\n", index + 1));
            render_hunk(&mut out, hunk, lang);
        }
    }

    out
}

fn render_hunk(out: &mut String, hunk: &ConflictHunk, lang: &str) {
    if !hunk.context_before.is_empty() {
        out.push_str("Context before:\n");
        out.push_str(&fenced_block(&hunk.context_before, lang));
    }
    out.push_str("Ours (new base):\n");
    out.push_str(&fenced_block(&hunk.ours, lang));
    if let Some(base) = &hunk.base {
        out.push_str("Base (common ancestor):\n");
        out.push_str(&fenced_block(base, lang));
    }
    out.push_str("Theirs (this commit):\n");
    out.push_str(&fenced_block(&hunk.theirs, lang));
    if !hunk.context_after.is_empty() {
        out.push_str("Context after:\n");
        out.push_str(&fenced_block(&hunk.context_after, lang));
    }
}

/// Wrap `content` in a code fence that is longer than any backtick run inside
/// it, so content containing fences can never break out of the framing.
fn fenced_block(content: &str, lang: &str) -> String {
    let longest_backtick_run = content.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat((longest_backtick_run + 1).max(3));
    format!("{fence}{lang}\n{content}\n{fence}\n\n")
}

/// Derive a fence language tag from the file extension, letters and digits only.
fn language_tag(path: &str) -> &str {
    let extension = path.rsplit('.').next().unwrap_or_default();
    if !extension.is_empty()
        && extension.len() < path.len()
        && extension.chars().all(|c| c.is_ascii_alphanumeric())
    {
        extension
    } else {
        ""
    }
}

/// Strip characters from a path that would break markdown headings.
fn sanitize_path(path: &str) -> String {
    path.chars()
        .filter(|c| !matches!(c, '\r' | '\n' | '`'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_grow_beyond_embedded_backticks() {
        let block = fenced_block("code with ```` four backticks", "rs");
        assert!(block.starts_with("`````rs\n"));
        assert!(block.trim_end().ends_with("`````"));
    }

    #[test]
    fn language_tags_are_validated() {
        assert_eq!(language_tag("src/main.rs"), "rs");
        assert_eq!(language_tag("Makefile"), "");
        assert_eq!(language_tag("weird.c++"), "");
    }
}
