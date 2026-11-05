//! What readers and agents say about a version of a brief: comments anchored to its blocks, its
//! lines or a passage, one answer each to what it asks, and which agents wait on them.
//!
//! The server's shapes aren't settled, so these read leniently too: what can't be read is left
//! out, rather than costing the rest.

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{Client, base, lenient, lenient_list, read_json, succeeded};

/// A thread of comments on a brief.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub id: String,
    /// The version it was said on.
    #[serde(default)]
    pub version: Option<u32>,
    pub actor: Actor,
    pub body: String,
    #[serde(default = "Anchor::other", deserialize_with = "anchor")]
    pub anchor: Anchor,
    #[serde(default, deserialize_with = "lenient")]
    pub resolved: Option<Resolved>,
    #[serde(default, deserialize_with = "lenient_list")]
    pub replies: Vec<Reply>,
}

/// Who said something: a person, or an agent on their behalf.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Actor {
    pub name: String,
    #[serde(default)]
    pub agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Resolved {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reply {
    pub actor: Actor,
    pub body: String,
}

/// What a comment is about. Blocks go by their name, `p1` being the version's first.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "on", rename_all = "lowercase")]
pub enum Anchor {
    Brief,
    Block {
        block: String,
    },
    Lines {
        block: String,
    },
    Text {
        #[serde(default)]
        block: Option<String>,
        quote: String,
    },
    /// A kind this client doesn't know yet.
    #[serde(other)]
    Other,
}

impl Anchor {
    fn other() -> Self {
        Self::Other
    }

    /// The 1-based position of the block it's about, when it's about one.
    pub fn block(&self) -> Option<usize> {
        let name = match self {
            Self::Block { block } | Self::Lines { block, .. } => Some(block),
            Self::Text { block, .. } => block.as_ref(),
            Self::Brief | Self::Other => None,
        };
        name?.strip_prefix('p')?.parse().ok()
    }
}

fn anchor<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Anchor, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(deserializer)?).unwrap_or(Anchor::Other))
}

/// The answers to what a version asks.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answers {
    #[serde(default, deserialize_with = "lenient_list")]
    pub by: Vec<Answered>,
    /// What whoever's asking answered.
    #[serde(default)]
    pub mine: Option<String>,
    /// Not on one's own brief.
    #[serde(default)]
    pub can_answer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Answered {
    pub name: String,
    pub answer: String,
}

/// An agent waiting on what's said about a brief.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Listener {
    pub name: String,
    #[serde(default)]
    pub agent: Option<String>,
}

/// What can be read of the list at `key` of `value`.
fn readable_list<T: serde::de::DeserializeOwned>(value: &Value, key: &str) -> Vec<T> {
    (value.get(key).and_then(Value::as_array))
        .map(|items| {
            (items.iter())
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// What's been said about a version, as it can be drawn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Activity {
    pub threads: Vec<Thread>,
    pub answers: Answers,
    pub listeners: Vec<Listener>,
    /// The brief's newest version, which can be newer than what found it says.
    pub latest: Option<u32>,
}

impl Activity {
    /// What the server's parts of it say, as much as can be read.
    pub fn read(raw: &RawActivity) -> Self {
        let threads = readable_list(&raw.comments, "comments");
        let listeners = readable_list(&raw.listeners, "listeners");
        let latest = (raw.versions.get("versions").and_then(Value::as_array))
            .and_then(|versions| {
                (versions.iter())
                    .filter_map(|version| version.get("version")?.as_u64())
                    .max()
            })
            .map(|latest| latest as u32);
        Self {
            threads,
            answers: serde_json::from_value(raw.answers.clone()).unwrap_or_default(),
            listeners,
            latest,
        }
    }
}

/// What's been said about a version, as the server sent it, for the cache.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RawActivity {
    #[serde(default)]
    pub comments: Value,
    #[serde(default)]
    pub answers: Value,
    #[serde(default)]
    pub listeners: Value,
    #[serde(default)]
    pub versions: Value,
}

/// A comment to post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewComment {
    pub body: String,
    pub to: CommentTo,
}

/// What a new comment is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentTo {
    /// The whole brief.
    Brief,
    /// A block, by its 1-based position.
    Block(usize),
    /// A passage of a block, as it reads on the page.
    Passage { block: usize, quote: String },
    /// Another thread.
    Thread(String),
}

impl NewComment {
    fn json(&self) -> Value {
        match &self.to {
            CommentTo::Brief => json!({ "body": self.body, "anchor": { "on": "brief" } }),
            CommentTo::Block(block) => json!({
                "body": self.body,
                "anchor": { "on": "block", "block": format!("p{block}") },
            }),
            CommentTo::Passage { block, quote } => json!({
                "body": self.body,
                "anchor": { "on": "text", "block": format!("p{block}"), "quote": quote },
            }),
            CommentTo::Thread(parent) => json!({ "body": self.body, "parent": parent }),
        }
    }
}

impl Client {
    /// What's been said about `version` of brief `id`. Reading records nothing and tells no one.
    pub async fn activity(&self, id: &str, version: u32) -> Result<RawActivity> {
        let of = |what: &str| format!("{}/api/briefs/{id}/{what}?version={version}", base());
        // Of every version, as what's said on an earlier one is what a newer one should answer.
        let comments = format!("{}/api/briefs/{id}/comments?version=all", base());
        let comments = read_json(self.get(&comments).send().await?).await?;
        let answers = read_json(self.get(&of("answers")).send().await?).await?;
        // Who's waiting and what's newest are worth having, not worth failing over.
        let worth_having = async |url: &str| match self.get(url).send().await {
            Ok(response) => read_json(response).await.unwrap_or_default(),
            Err(_) => Value::Null,
        };
        let listeners = worth_having(&of("listeners")).await;
        let versions = worth_having(&format!("{}/api/briefs/{id}/versions", base())).await;
        Ok(RawActivity {
            comments,
            answers,
            listeners,
            versions,
        })
    }

    /// Post `comment` on `version` of brief `id`, for its watchers and the agents waiting on it.
    pub async fn post_comment(&self, id: &str, version: u32, comment: &NewComment) -> Result<()> {
        let url = format!("{}/api/briefs/{id}/comments?version={version}", base());
        let posted = (self.http.post(url))
            .header("X-Auth-Token", &self.token.0)
            .json(&comment.json())
            .send()
            .await
            .context("Couldn't reach Briefs")?;
        succeeded(posted)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_of_an_anchor_not_known_yet_still_reads() {
        let thread: Thread = serde_json::from_value(json!({
            "id": "c1",
            "actor": { "kind": "direct", "name": "Ada" },
            "body": "Why this?",
            "anchor": { "on": "frame", "at": 3 },
            "replies": "not a list",
        }))
        .unwrap();
        assert_eq!(thread.anchor, Anchor::Other, "a new kind still reads");
        assert!(thread.replies.is_empty(), "what can't be read is left out");
    }

    #[test]
    fn an_anchor_says_which_block_it_is_about() {
        let text: Anchor =
            serde_json::from_value(json!({ "on": "text", "block": "p4", "quote": "q" })).unwrap();
        assert_eq!(text.block(), Some(4));
        assert_eq!(
            Anchor::Brief.block(),
            None,
            "the whole brief isn't one block"
        );
    }

    #[test]
    fn what_a_comment_is_on_goes_by_the_servers_names() {
        let on = |to| {
            NewComment {
                body: "b".into(),
                to,
            }
            .json()
        };
        assert_eq!(
            on(CommentTo::Block(3)),
            json!({ "body": "b", "anchor": { "on": "block", "block": "p3" } }),
        );
        assert_eq!(
            on(CommentTo::Passage {
                block: 2,
                quote: "q".into()
            }),
            json!({ "body": "b", "anchor": { "on": "text", "block": "p2", "quote": "q" } }),
        );
        assert_eq!(
            on(CommentTo::Thread("c1".into())),
            json!({ "body": "b", "parent": "c1" }),
        );
    }
}
