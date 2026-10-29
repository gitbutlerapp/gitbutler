use serde::Serialize;

const PREVIEW_TEXT_CHARS: usize = 320;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct ObservedTargetKeys {
    pub(crate) branches: Vec<String>,
    pub(crate) reviews: Vec<String>,
    pub(crate) changes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RecordPreview {
    pub(crate) timestamp: Option<String>,
    pub(crate) source_event_kind: Option<String>,
    pub(crate) text: String,
}

pub(super) fn push_unique(values: &mut Vec<String>, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_owned());
    }
}

pub(super) fn compact_preview(text: &str) -> String {
    let trimmed = text.trim();
    let mut chars = trimmed.chars();
    let mut preview = chars.by_ref().take(PREVIEW_TEXT_CHARS).collect::<String>();
    if chars.next().is_some() {
        preview.push_str("...");
    }
    preview
}
