//! A thin client of the Briefs API at `gitbutler.com/briefs`: what a git client reads to find
//! and show the briefs about what it shows, as the person a GitButler account token is for.
//!
//! Briefs calls this API experimental: it has no version, and any shape may change on any day.
//! So unknown fields are ignored, only what nothing can be shown without is required, and a
//! block that can't be read becomes [`BlockKind::Unknown`], which a caller links to the web for.

use std::time::Duration;

use anyhow::{Result, bail, ensure};
use but_secret::Sensitive;
use reqwest::{RequestBuilder, Response, StatusCode, redirect};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Where Briefs is, and the only place the token goes.
const BASE: &str = "https://gitbutler.com/briefs";

/// The most commits one [`Client::find`] may name.
pub const MOST_COMMITS: usize = 100;

/// The page of `version` of brief `id`, for a browser.
pub fn web_url(id: &str, version: u32) -> String {
    format!("{BASE}/b/{id}/v/{version}")
}

/// Where a media file of brief `id` is, for [`Client::media`].
pub fn media_url(id: &str, sha: &str) -> String {
    format!("{BASE}/b/{id}/media/{sha}")
}

/// Whether `url` is one [`media_url`] made.
pub fn is_media_url(url: &str) -> bool {
    url.strip_prefix(BASE)
        .is_some_and(|path| path.starts_with("/b/") && path.contains("/media/"))
}

/// Reads Briefs as the person a GitButler account token is for.
pub struct Client {
    http: reqwest::Client,
    token: Sensitive<String>,
}

impl Client {
    pub fn new(token: Sensitive<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            // HTTP clients keep custom headers across hosts, and media redirects to an image
            // host, so nothing is followed with the token on: `media` follows by hand.
            .redirect(redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { http, token })
    }

    fn get(&self, url: &str) -> RequestBuilder {
        self.http.get(url).header("X-Auth-Token", &self.token.0)
    }

    /// The briefs of the repository at `remote`, in any form git gives it, about `branch` and
    /// `commits`, which take full ids. Each brief comes once, by its version that matches best,
    /// the strongest matches first. A brief that can't be read is left out.
    pub async fn find(&self, remote: &str, branch: &str, commits: &[String]) -> Result<Vec<Found>> {
        #[derive(Deserialize)]
        struct Answer {
            briefs: Vec<Value>,
        }
        let query = [("remote", remote), ("branch", branch)]
            .into_iter()
            .chain(commits.iter().map(|commit| ("commit", commit.as_str())))
            .collect::<Vec<_>>();
        let url = format!("{BASE}/api/briefs");
        let answer: Answer = read_json(self.get(&url).query(&query).send().await?).await?;
        Ok((answer.briefs.into_iter())
            .filter_map(|found| serde_json::from_value(found).ok())
            .collect())
    }

    /// `version` of brief `id`. A published version never changes.
    pub async fn read(&self, id: &str, version: u32) -> Result<Brief> {
        let url = format!("{BASE}/api/briefs/{id}/v/{version}");
        read_json(self.get(&url).send().await?).await
    }

    /// What `url`, from [`media_url`], serves. Briefs checks the token and redirects to where
    /// the file is, which is asked without it.
    pub async fn media(&self, url: &str) -> Result<Vec<u8>> {
        ensure!(is_media_url(url), "Not a brief's media file");
        let response = self.get(url).send().await?;
        let response = match response.status().is_redirection() {
            false => response,
            true => {
                let place = response.headers().get(reqwest::header::LOCATION);
                let place = place.and_then(|place| place.to_str().ok());
                let Some(place) = place.filter(|place| place.starts_with("https://")) else {
                    bail!("Briefs redirected a media file somewhere unexpected");
                };
                self.http.get(place).send().await?
            }
        };
        Ok(succeeded(response)?.bytes().await?.to_vec())
    }
}

/// `response`, when it says it worked. The status alone, as a request's address names the
/// repository and its branches, which logs shouldn't.
fn succeeded(response: Response) -> Result<Response> {
    match response.status() {
        StatusCode::UNAUTHORIZED => bail!("Briefs didn't accept the sign-in"),
        status if !status.is_success() => bail!("Briefs answered {status}"),
        _ => Ok(response),
    }
}

async fn read_json<T: DeserializeOwned>(response: Response) -> Result<T> {
    Ok(succeeded(response)?.json().await?)
}

/// A brief about what a git client shows, and how.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Found {
    pub id: String,
    pub author: Author,
    /// Its latest published version, which may be newer than the one that matched.
    pub latest: u32,
    #[serde(rename = "match")]
    pub matched: Match,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Author {
    pub name: String,
}

/// The version of a brief that matched.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub how: How,
    pub version: u32,
    /// The commit that version's code was committed up to, when it's known.
    pub head: Option<String>,
    /// That version had uncommitted changes on top of `head` too.
    #[serde(default)]
    pub uncommitted: bool,
}

/// How a brief is about what was asked, strongest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum How {
    /// That version was computed at one of the commits given.
    Head,
    /// That version names one of the commits given, which can be no more than its base: the
    /// commit its change starts from, and so not what it's about.
    Commit,
    /// Only the branch's name matches.
    Branch,
    /// A kind this client doesn't know yet.
    #[serde(other)]
    Other,
}

/// A version of a brief.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Brief {
    pub created_at: Option<String>,
    pub review: Review,
}

/// What a brief says.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Review {
    pub title: String,
    /// What its comparison changes in all.
    pub changed: Option<Changed>,
    #[serde(deserialize_with = "blocks")]
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Changed {
    pub files: u32,
    pub additions: u32,
    pub deletions: u32,
}

/// A part of a brief, in the order it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The one line that says what's folded here, when it's folded away at first. Neighbouring
    /// blocks with the same one fold together.
    pub detail: Option<String>,
    pub kind: BlockKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum BlockKind {
    Text {
        markdown: String,
    },
    Excerpt {
        path: String,
        range: LineSpan,
        lines: Vec<String>,
    },
    Diff {
        path: String,
        hunks: Vec<DiffHunk>,
    },
    Media {
        media: String,
        #[serde(rename = "type")]
        media_type: String,
        /// A few words over it that say which it is, as "Before".
        title: Option<String>,
        width: u32,
        height: u32,
        alt: String,
    },
    /// A kind this client can't read, by its name: the web shows it.
    #[serde(skip)]
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct LineSpan {
    pub start: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: u32,
    pub new_start: u32,
    /// Git's hunk lines, each after a space, `+` or `-`, with no `@@` header.
    pub body: String,
}

/// Each block one at a time, so one that can't be read costs only itself.
fn blocks<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Block>, D::Error> {
    let blocks = Vec::<Value>::deserialize(deserializer)?;
    Ok(blocks.into_iter().map(read_block).collect())
}

fn read_block(block: Value) -> Block {
    let text = |field: &str| block.get(field).and_then(Value::as_str).map(str::to_owned);
    let detail = text("detail");
    let name = text("kind").unwrap_or_default();
    let kind = serde_json::from_value(block).unwrap_or(BlockKind::Unknown(name));
    Block { detail, kind }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_block_that_cannot_be_read_costs_only_itself() {
        let review: Review = serde_json::from_value(json!({
            "title": "t",
            "blocks": [
                { "kind": "text", "markdown": "first", "future": 1 },
                { "kind": "poll", "detail": "Vote", "options": [] },
                { "kind": "diff", "path": "a.rs", "hunks": "not a list" },
            ],
        }))
        .unwrap();
        assert_eq!(
            review.blocks,
            [
                Block {
                    detail: None,
                    kind: BlockKind::Text {
                        markdown: "first".into()
                    },
                },
                Block {
                    detail: Some("Vote".into()),
                    kind: BlockKind::Unknown("poll".into()),
                },
                Block {
                    detail: None,
                    kind: BlockKind::Unknown("diff".into()),
                },
            ]
        );
    }

    #[test]
    fn a_match_of_a_kind_not_known_yet_still_reads() {
        let found: Found = serde_json::from_value(json!({
            "id": "b1",
            "title": "t",
            "author": { "name": "Ada", "picture": null },
            "access": "shared",
            "latest": 3,
            "match": {
                "how": "change",
                "version": 2,
                "head": null,
                "publishedAt": "2026-11-04T12:01:35.551Z",
            },
        }))
        .unwrap();
        assert_eq!(found.matched.how, How::Other, "a new kind still reads");
        assert!(!found.matched.uncommitted, "a missing field is its default");
    }

    #[test]
    fn only_a_brief_s_media_counts_as_media() {
        assert!(
            is_media_url(&media_url("b1", "abc")),
            "what this crate makes is what it may send the token to"
        );
        for other in [
            "https://gitbutler.com/briefs/api/briefs",
            "https://gitbutler.com.evil.example/briefs/b/b1/media/abc",
            "http://gitbutler.com/briefs/b/b1/media/abc",
            "https://github.com/briefs/b/b1/media/abc",
        ] {
            assert!(
                !is_media_url(other),
                "{other} isn't Briefs' media, so the token stays away"
            );
        }
    }
}
