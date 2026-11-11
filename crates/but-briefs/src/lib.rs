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

mod cache;
pub use cache::{Anchored, Cache, Note};
mod review;
pub use review::{
    Activity, Actor, Anchor, Answered, Answers, CommentTo, Listener, NewComment, RawActivity,
    Reply, Thread,
};

/// Where Briefs is, and the only place the token goes. A debug build can be pointed at a stand-in
/// with `GITBUTLER_BRIEFS_BASE`, to try what writes without touching anyone's briefs.
fn base() -> String {
    const BASE: &str = "https://gitbutler.com/briefs";
    #[cfg(debug_assertions)]
    if let Ok(base) = std::env::var("GITBUTLER_BRIEFS_BASE") {
        return base;
    }
    BASE.to_owned()
}

/// The most commits one `find` may name.
pub const MOST_COMMITS: usize = 100;

/// The most branches one [`Client::find`] may name.
const MOST_BRANCHES: usize = 10;

/// The page of `version` of brief `id`, for a browser.
pub fn web_url(id: &str, version: u32) -> String {
    format!("{}/b/{id}/v/{version}", base())
}

/// Where a media file of brief `id` is, for [`Client::media`].
pub fn media_url(id: &str, sha: &str) -> String {
    format!("{}/b/{id}/media/{sha}", base())
}

/// Whether `url` is one [`media_url`] made.
pub fn is_media_url(url: &str) -> bool {
    url.strip_prefix(&base())
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

    /// The briefs of the repository at `remote`, in any form git gives it, about `branches` and
    /// `commits`, which take full ids, as the server sent them. Each brief comes once, by its
    /// version that matches best, the strongest matches first.
    async fn find(&self, remote: &str, branches: &[&str], commits: &[&str]) -> Result<Vec<Value>> {
        #[derive(Deserialize)]
        struct Answer {
            briefs: Vec<Value>,
        }
        let query = std::iter::once(("remote", remote))
            .chain(branches.iter().map(|branch| ("branch", *branch)))
            .chain(commits.iter().map(|commit| ("commit", *commit)))
            .collect::<Vec<_>>();
        let url = format!("{}/api/briefs", base());
        let answer: Answer = read_json(self.get(&url).query(&query).send().await?).await?;
        Ok(answer.briefs)
    }

    /// `version` of brief `id`, as the server sent it. A published version never changes.
    pub async fn read(&self, id: &str, version: u32) -> Result<FetchedVersion> {
        let url = format!("{}/api/briefs/{id}/v/{version}", base());
        Ok(FetchedVersion {
            id: id.to_owned(),
            version,
            body: read_json(self.get(&url).send().await?).await?,
        })
    }

    /// Ask about `chunk` of the branches of the repository at `remote`, one of [`chunks`], and
    /// read the versions that matched that `known_branch` doesn't know the branch of yet. A
    /// brief that only names a commit isn't about the branch: that can be no more than where
    /// another change starts from.
    pub async fn ask(
        &self,
        remote: &str,
        chunk: &[BranchAsk],
        known_branch: impl Fn(&str, u32) -> Option<String>,
    ) -> Result<Asked> {
        let branches: Vec<&str> = chunk.iter().map(|ask| ask.branch.as_str()).collect();
        let commits: Vec<&str> = (chunk.iter())
            .flat_map(|ask| ask.commits.iter().map(String::as_str))
            .collect();
        let mut versions = Vec::new();
        let mut matches = Vec::new();
        for value in self.find(remote, &branches, &commits).await? {
            let Ok(found) = serde_json::from_value::<Found>(value.clone()) else {
                continue;
            };
            if found.matched.how == How::Commit {
                continue;
            }
            let version = found.matched.version;
            let written_on = match known_branch(&found.id, version) {
                Some(branch) => Some(branch),
                // One that can't be read leaves the rest to be told about.
                None => match self.read(&found.id, version).await {
                    Ok(read) => {
                        let branch = read.body.pointer("/review/repo/branch");
                        let branch = branch.and_then(Value::as_str).map(str::to_owned);
                        versions.push(read);
                        branch
                    }
                    Err(err) => {
                        tracing::debug!("couldn't read {}: {err:#}", found.id);
                        None
                    }
                },
            };
            matches.push(Matched {
                found: value,
                head: found.matched.head,
                written_on,
            });
        }
        Ok(Asked {
            anchors: anchors_of(chunk, &matches),
            versions,
        })
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchAsk {
    pub branch: String,
    /// Its commits, newest first, by their full ids.
    pub commits: Vec<String>,
}

/// `asks` in the groups one request can name.
pub fn chunks(asks: &[BranchAsk]) -> Vec<&[BranchAsk]> {
    let mut chunks = Vec::new();
    let mut rest = asks;
    while !rest.is_empty() {
        let mut commits = 0;
        let fit = (rest.iter().take(MOST_BRANCHES))
            .take_while(|ask| {
                commits += ask.commits.len();
                commits <= MOST_COMMITS
            })
            .count()
            // One branch always goes, as many commits as it has: asking cuts them to the most.
            .max(1);
        let (chunk, after) = rest.split_at(fit);
        chunks.push(chunk);
        rest = after;
    }
    chunks
}

/// A version of a brief as the server sent it.
#[derive(Debug, Clone, PartialEq)]
pub struct FetchedVersion {
    pub id: String,
    pub version: u32,
    pub body: Value,
}

/// What asking about some branches said, for [`Cache::took`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Asked {
    /// Each branch asked about, with what's about it as the server sent it, the best first.
    pub anchors: Vec<(String, Vec<Value>)>,
    /// The versions that had to be read.
    pub versions: Vec<FetchedVersion>,
}

/// A brief an answer holds, with what says which branches it's about.
struct Matched {
    found: Value,
    head: Option<String>,
    written_on: Option<String>,
}

/// Each branch of `chunk` with what's about it. The server doesn't say which of the branches
/// asked about a brief matched, so that's where it was written: at one of a branch's commits, or
/// on a branch of its name.
fn anchors_of(chunk: &[BranchAsk], matches: &[Matched]) -> Vec<(String, Vec<Value>)> {
    (chunk.iter())
        .map(|ask| {
            let found = (matches.iter())
                .filter(|found| {
                    found
                        .head
                        .as_ref()
                        .is_some_and(|head| ask.commits.contains(head))
                        || found.written_on.as_ref() == Some(&ask.branch)
                })
                .map(|found| found.found.clone())
                .collect();
            (ask.branch.clone(), found)
        })
        .collect()
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
    /// Its latest version's title.
    #[serde(default)]
    pub title: String,
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
    /// What its CLI compared.
    #[serde(default, deserialize_with = "lenient")]
    pub comparison: Option<Comparison>,
    /// What its comparison changes in all, which its CLI measured.
    #[serde(default, deserialize_with = "lenient")]
    pub changed: Option<Changed>,
    /// What its author wants of who reads it. None asks for nothing in particular.
    #[serde(default, deserialize_with = "lenient")]
    pub ask: Option<Ask>,
    /// What its CLI measured about the change, none of it written by the agent.
    #[serde(default, deserialize_with = "lenient_list")]
    pub labels: Vec<Label>,
    #[serde(default, deserialize_with = "lenient")]
    pub generator: Option<Generator>,
    #[serde(deserialize_with = "blocks")]
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Comparison {
    /// The commit its code was committed up to, when that's known.
    #[serde(default)]
    pub head: Option<String>,
    /// It had uncommitted changes on top of `head` too.
    #[serde(default)]
    pub uncommitted: bool,
}

/// What an author wants of who reads a brief.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Ask {
    SignOff,
    HeadsUp,
    Decision {
        #[serde(default)]
        options: Vec<String>,
    },
    /// A kind this client doesn't know yet.
    #[serde(other)]
    Other,
}

/// One thing a brief's CLI measured about the change.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Label {
    pub name: String,
    pub why: String,
    pub concern: Concern,
}

/// How much a [`Label`] asks of a reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Concern {
    None,
    Notable,
    Serious,
    /// A kind this client doesn't know yet.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Generator {
    /// The agent that wrote it, in its own words.
    pub agent: Option<String>,
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
        /// The commit it's read at, or `worktree`.
        #[serde(default)]
        at: String,
        path: String,
        range: LineSpan,
        lines: Vec<String>,
    },
    Diff {
        #[serde(default, deserialize_with = "lenient")]
        comparison: Option<BlockComparison>,
        path: String,
        /// `modified`, `added`, `deleted` or `renamed`. None: the file didn't change, and all of
        /// it is context.
        #[serde(default)]
        status: Option<String>,
        /// The whole file's, not what the hunks show.
        #[serde(default, deserialize_with = "lenient")]
        stats: Option<LineStats>,
        hunks: Vec<DiffHunk>,
        /// What isn't shown, in the words that follow "Not shown:".
        #[serde(default, deserialize_with = "lenient_list")]
        omitted: Vec<String>,
    },
    /// The files of a comparison, each as git says.
    Files {
        #[serde(default, deserialize_with = "lenient_list")]
        files: Vec<ChangedFile>,
        #[serde(default, deserialize_with = "lenient_list")]
        omitted: Vec<String>,
    },
    Media {
        media: String,
        #[serde(rename = "type")]
        media_type: String,
        /// A few words over it that say which it is, as "Before".
        title: Option<String>,
        /// Set on neighbours meant to be seen together, as a before and an after.
        group: Option<String>,
        width: u32,
        height: u32,
        alt: String,
    },
    /// A kind this client can't read, by its name: the web shows it.
    #[serde(skip)]
    Unknown(String),
}

/// What a block compares. `head` is a commit, `worktree`, or `proposal`, whose added lines the
/// agent wrote and which exist nowhere yet.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BlockComparison {
    #[serde(default)]
    pub head: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct LineStats {
    #[serde(default)]
    pub additions: u32,
    #[serde(default)]
    pub deletions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChangedFile {
    #[serde(default)]
    pub status: Option<String>,
    pub path: String,
    #[serde(default, deserialize_with = "lenient")]
    pub stats: Option<LineStats>,
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

/// What's there if it can be read, so a field that changed shape costs only itself.
fn lenient<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(deserializer)?).ok())
}

/// Each of a list that can be read.
fn lenient_list<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    let list = Option::<Vec<Value>>::deserialize(deserializer).unwrap_or_default();
    Ok((list.into_iter().flatten())
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect())
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

    fn ask(branch: &str, commits: &[&str]) -> BranchAsk {
        BranchAsk {
            branch: branch.into(),
            commits: commits.iter().map(|commit| (*commit).into()).collect(),
        }
    }

    fn matched(id: &str, head: Option<&str>, written_on: Option<&str>) -> Matched {
        Matched {
            found: json!({ "id": id }),
            head: head.map(str::to_owned),
            written_on: written_on.map(str::to_owned),
        }
    }

    #[test]
    fn each_branch_of_a_batch_gets_only_the_briefs_about_it() {
        let chunk = [
            ask("a", &["a2", "a1"]),
            ask("b", &["b1"]),
            ask("c", &["c1"]),
        ];
        let anchors = anchors_of(
            &chunk,
            &[
                matched("at-a-commit", Some("a1"), Some("gitbutler/workspace")),
                matched("by-its-name", Some("gone"), Some("b")),
                matched("elsewhere", Some("gone"), Some("other")),
            ],
        );
        assert_eq!(
            anchors,
            [
                ("a".to_owned(), vec![json!({ "id": "at-a-commit" })]),
                ("b".to_owned(), vec![json!({ "id": "by-its-name" })]),
                ("c".to_owned(), Vec::new()),
            ],
            "written at a branch's commit or on its name, and the rest have none"
        );
    }

    #[test]
    fn a_batch_is_as_much_as_one_request_may_name() {
        let many: Vec<_> = (0..23).map(|n| ask(&n.to_string(), &["tip"])).collect();
        assert_eq!(
            chunks(&many)
                .iter()
                .map(|chunk| chunk.len())
                .collect::<Vec<_>>(),
            [10, 10, 3],
            "at most 10 branches"
        );
        let commits = vec!["c"; 60];
        let long = [ask("a", &commits), ask("b", &commits), ask("c", &["tip"])];
        assert_eq!(
            chunks(&long)
                .iter()
                .map(|chunk| chunk.len())
                .collect::<Vec<_>>(),
            [1, 2],
            "at most 100 commits"
        );
        assert!(chunks(&[]).is_empty(), "nothing to ask, nothing to send");
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
