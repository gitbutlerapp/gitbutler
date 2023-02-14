use crate::{butler, fs, sessions};
use difference::{Changeset, Difference};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path, time::SystemTime};
use yrs::{Doc, GetString, Text, Transact};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Delta {
    operations: Vec<Operation>,
    timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Operation {
    // corresponds to YText.insert(index, chunk)
    Insert((u32, String)),
    // corresponds to YText.remove_range(index, len)
    Delete((u32, u32)),
}

fn get_delta_operations(initial_text: &str, final_text: &str) -> Vec<Operation> {
    if initial_text == final_text {
        return vec![];
    }

    let changeset = Changeset::new(initial_text, final_text, "");
    let mut offset: u32 = 0;
    let mut deltas = vec![];

    for edit in changeset.diffs {
        match edit {
            Difference::Rem(text) => {
                deltas.push(Operation::Delete((offset, text.len() as u32)));
            }
            Difference::Add(text) => {
                deltas.push(Operation::Insert((offset, text.to_string())));
            }
            Difference::Same(text) => {
                offset += text.len() as u32;
            }
        }
    }

    return deltas;
}

#[derive(Debug, Clone, Default)]
pub struct TextDocument {
    doc: Doc,
    deltas: Vec<Delta>,
}

const TEXT_DOCUMENT_NAME: &str = "document";

impl TextDocument {
    fn apply_deltas(doc: &Doc, deltas: &Vec<Delta>) {
        if deltas.len() == 0 {
            return;
        }
        let text = doc.get_or_insert_text(TEXT_DOCUMENT_NAME);
        let mut txn = doc.transact_mut();
        for event in deltas {
            for operation in event.operations.iter() {
                match operation {
                    Operation::Insert((index, chunk)) => {
                        text.insert(&mut txn, *index, chunk);
                    }
                    Operation::Delete((index, len)) => {
                        text.remove_range(&mut txn, *index, *len);
                    }
                }
            }
        }
    }

    // creates a new text document from a deltas.
    pub fn from_deltas(deltas: Vec<Delta>) -> TextDocument {
        let doc = Doc::new();
        Self::apply_deltas(&doc, &deltas);
        TextDocument {
            doc: doc.clone(),
            deltas,
        }
    }

    pub fn get_deltas(&self) -> Vec<Delta> {
        self.deltas.clone()
    }

    // returns a text document where internal state is seeded with value, and deltas are applied.
    pub fn new(value: &str, deltas: Vec<Delta>) -> TextDocument {
        let doc = Doc::new();
        let mut all_deltas = vec![Delta {
            operations: get_delta_operations("", value),
            timestamp_ms: 0,
        }];
        all_deltas.append(&mut deltas.clone());
        Self::apply_deltas(&doc, &all_deltas);
        TextDocument {
            doc: doc.clone(),
            deltas,
        }
    }

    pub fn update(&mut self, value: &str) -> bool {
        let diffs = get_delta_operations(&self.to_string(), value);
        let event = Delta {
            operations: diffs,
            timestamp_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        if event.operations.len() == 0 {
            return false;
        }
        Self::apply_deltas(&self.doc, &vec![event.clone()]);
        self.deltas.push(event);
        return true;
    }

    pub fn to_string(&self) -> String {
        let doc = &self.doc;
        let text = doc.get_or_insert_text(TEXT_DOCUMENT_NAME);
        let txn = doc.transact();
        text.get_string(&txn)
    }
}

#[derive(Debug)]
pub enum ErrorCause {
    IOError(std::io::Error),
    ParseJSONError(serde_json::Error),
    GitError(git2::Error),
    StringError(String),
    SessionError(sessions::Error),
    SessionNotFound,
}

impl From<sessions::Error> for ErrorCause {
    fn from(e: sessions::Error) -> Self {
        ErrorCause::SessionError(e)
    }
}

impl From<String> for ErrorCause {
    fn from(e: String) -> Self {
        ErrorCause::StringError(e)
    }
}

impl From<git2::Error> for ErrorCause {
    fn from(e: git2::Error) -> Self {
        ErrorCause::GitError(e)
    }
}

impl From<std::io::Error> for ErrorCause {
    fn from(e: std::io::Error) -> Self {
        ErrorCause::IOError(e)
    }
}

impl From<serde_json::Error> for ErrorCause {
    fn from(e: serde_json::Error) -> Self {
        ErrorCause::ParseJSONError(e)
    }
}

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub cause: ErrorCause,
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.cause {
            ErrorCause::IOError(ref e) => Some(e),
            ErrorCause::ParseJSONError(ref e) => Some(e),
            ErrorCause::GitError(ref e) => Some(e),
            ErrorCause::StringError(_) => None,
            ErrorCause::SessionError(ref e) => Some(e),
            ErrorCause::SessionNotFound => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.cause {
            ErrorCause::IOError(ref e) => write!(f, "{}: {}", self.message, e),
            ErrorCause::ParseJSONError(ref e) => write!(f, "{}: {}", self.message, e),
            ErrorCause::GitError(ref e) => write!(f, "{}: {}", self.message, e),
            ErrorCause::StringError(ref e) => write!(f, "{}: {}", self.message, e),
            ErrorCause::SessionError(ref e) => write!(f, "{}: {}", self.message, e),
            ErrorCause::SessionNotFound => write!(f, "{}", self.message),
        }
    }
}

pub fn get_current_file_deltas(
    repo: &git2::Repository,
    file_path: &Path,
) -> Result<Option<Vec<Delta>>, Error> {
    let deltas_path = repo.path().join(butler::dir()).join("session/deltas");
    let file_deltas_path = deltas_path.join(file_path);
    if !file_deltas_path.exists() {
        return Ok(None);
    }

    let file_deltas = std::fs::read_to_string(&file_deltas_path).map_err(|e| Error {
        message: format!(
            "Could not read delta file at {}",
            file_deltas_path.display()
        ),
        cause: e.into(),
    });

    match file_deltas {
        Ok(file_deltas) => {
            let file_deltas: Vec<Delta> =
                serde_json::from_str(&file_deltas).map_err(|e| Error {
                    message: format!(
                        "Could not parse delta file at {}",
                        file_deltas_path.display()
                    ),
                    cause: e.into(),
                })?;
            Ok(Some(file_deltas))
        }
        Err(err) => Err(err),
    }
}

pub fn save_current_file_deltas(
    repo: &git2::Repository,
    file_path: &Path,
    deltas: &Vec<Delta>,
) -> Result<(), std::io::Error> {
    let project_deltas_path = repo.path().join(butler::dir()).join("session/deltas");
    let delta_path = project_deltas_path.join(file_path);
    let delta_dir = delta_path.parent().unwrap();
    std::fs::create_dir_all(&delta_dir)?;
    log::info!("mkdir {}", delta_path.to_str().unwrap());
    log::info!("Writing deltas to {}", delta_path.to_str().unwrap());
    let raw_deltas = serde_json::to_string(&deltas)?;
    std::fs::write(delta_path, raw_deltas)?;
    Ok(())
}

// returns deltas for a current session from .gb/session/deltas tree
fn list_current_deltas(repo: &git2::Repository) -> Result<HashMap<String, Vec<Delta>>, Error> {
    let deltas_path = repo.path().join(butler::dir()).join("session/deltas");
    if !deltas_path.exists() {
        return Ok(HashMap::new());
    }

    let file_paths = fs::list_files(&deltas_path).map_err(|e| Error {
        message: format!("Could not list delta files at {}", deltas_path.display()),
        cause: e.into(),
    })?;

    let deltas = file_paths
        .iter()
        .map_while(|file_path| {
            let file_deltas = get_current_file_deltas(repo, Path::new(file_path));
            match file_deltas {
                Ok(Some(file_deltas)) => Some(Ok((file_path.to_owned(), file_deltas))),
                Ok(None) => None,
                Err(err) => Some(Err(err)),
            }
        })
        .collect::<Result<HashMap<String, Vec<Delta>>, Error>>()?;

    Ok(deltas)
}

pub fn list(
    repo: &git2::Repository,
    session_id: &str,
) -> Result<HashMap<String, Vec<Delta>>, Error> {
    let session = match sessions::get(repo, session_id).map_err(|e| Error {
        message: format!("Could not get session {}", session_id),
        cause: e.into(),
    })? {
        Some(session) => session,
        None => Err(Error {
            message: format!("Could not find session {}", session_id),
            cause: ErrorCause::SessionNotFound,
        })?,
    };

    if session.hash.is_none() {
        list_current_deltas(repo)
    } else {
        list_commit_deltas(repo, &session.hash.unwrap())
    }
}

// returns deltas from gitbutler commit's session/deltas tree
pub fn list_commit_deltas(
    repo: &git2::Repository,
    commit_hash: &str,
) -> Result<HashMap<String, Vec<Delta>>, Error> {
    let commit_id = git2::Oid::from_str(commit_hash).map_err(|e| Error {
        message: format!("Could not parse commit id {}", commit_hash),
        cause: e.into(),
    })?;
    let commit = repo.find_commit(commit_id).map_err(|e| Error {
        message: format!("Could not find commit {}", commit_id),
        cause: e.into(),
    })?;

    let tree = commit.tree().map_err(|e| Error {
        message: format!("Could not get tree for commit {}", commit.id()),
        cause: e.into(),
    })?;

    let mut blobs = HashMap::new();

    tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
        if entry.name().is_none() {
            return git2::TreeWalkResult::Ok;
        }
        let entry_path = Path::new(root).join(entry.name().unwrap());
        if !entry_path.starts_with("session/deltas") {
            return git2::TreeWalkResult::Ok;
        }
        if entry.kind() != Some(git2::ObjectType::Blob) {
            return git2::TreeWalkResult::Ok;
        }
        let blob = entry.to_object(repo).and_then(|obj| obj.peel_to_blob());
        let content = blob.map(|blob| blob.content().to_vec());

        match content {
            Ok(content) => {
                let deltas: Result<Vec<Delta>, Error> =
                    serde_json::from_slice(&content).map_err(|e| Error {
                        message: format!("Could not parse delta file at {}", entry_path.display()),
                        cause: e.into(),
                    });
                blobs.insert(
                    entry_path
                        .strip_prefix("session/deltas")
                        .unwrap()
                        .to_owned(),
                    deltas,
                );
            }
            Err(e) => {
                log::error!("Could not get blob for {}: {}", entry_path.display(), e);
            }
        }
        git2::TreeWalkResult::Ok
    })
    .map_err(|e| Error {
        message: format!("Could not walk tree for commit {}", commit.id()),
        cause: e.into(),
    })?;

    if let Some(error) = blobs.values().find_map(|blob| blob.as_ref().err()) {
        Err(Error {
            message: "Could not get all deltas".to_owned(),
            cause: error.to_string().into(),
        })
    } else {
        let deltas = blobs
            .into_iter()
            .map(|(path, deltas)| (path.to_str().unwrap().to_owned(), deltas.unwrap()))
            .collect();
        Ok(deltas)
    }
}

#[test]
fn test_get_delta_operations_insert_end() {
    let initial_text = "hello world";
    let final_text = "hello world!";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Insert((11, "!".to_string())));
}

#[test]
fn test_get_delta_operations_insert_middle() {
    let initial_text = "hello world";
    let final_text = "hello, world";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Insert((5, ",".to_string())));
}

#[test]
fn test_get_delta_operations_insert_begin() {
    let initial_text = "hello world";
    let final_text = ": hello world";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Insert((0, ": ".to_string())));
}

#[test]
fn test_get_delta_operations_delete_end() {
    let initial_text = "hello world!";
    let final_text = "hello world";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Delete((11, 1)));
}

#[test]
fn test_get_delta_operations_delete_middle() {
    let initial_text = "hello world";
    let final_text = "helloworld";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Delete((5, 1)));
}

#[test]
fn test_get_delta_operations_delete_begin() {
    let initial_text = "hello world";
    let final_text = "ello world";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Delete((0, 1)));
}

#[test]
fn test_document_new() {
    let document = TextDocument::new("hello world", vec![]);
    assert_eq!(document.to_string(), "hello world");
    assert_eq!(document.get_deltas().len(), 0);
}

#[test]
fn test_document_update() {
    let mut document = TextDocument::new("hello world", vec![]);
    document.update("hello world!");
    assert_eq!(document.to_string(), "hello world!");
    assert_eq!(document.get_deltas().len(), 1);
    assert_eq!(document.get_deltas()[0].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[0].operations[0],
        Operation::Insert((11, "!".to_string()))
    );
}

#[test]
fn test_document_empty() {
    let mut document = TextDocument::from_deltas(vec![]);
    document.update("hello world!");
    assert_eq!(document.to_string(), "hello world!");
    assert_eq!(document.get_deltas().len(), 1);
    assert_eq!(document.get_deltas()[0].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[0].operations[0],
        Operation::Insert((0, "hello world!".to_string()))
    );
}

#[test]
fn test_document_from_deltas() {
    let document = TextDocument::from_deltas(vec![
        Delta {
            timestamp_ms: 0,
            operations: vec![Operation::Insert((0, "hello".to_string()))],
        },
        Delta {
            timestamp_ms: 1,
            operations: vec![Operation::Insert((5, " world".to_string()))],
        },
        Delta {
            timestamp_ms: 2,
            operations: vec![
                Operation::Delete((3, 7)),
                Operation::Insert((4, "!".to_string())),
            ],
        },
    ]);
    assert_eq!(document.to_string(), "held!");
}

#[test]
fn test_document_complex() {
    let mut document = TextDocument::from_deltas(vec![]);

    document.update("hello");
    assert_eq!(document.to_string(), "hello");
    assert_eq!(document.get_deltas().len(), 1);
    assert_eq!(document.get_deltas()[0].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[0].operations[0],
        Operation::Insert((0, "hello".to_string()))
    );

    document.update("hello world");
    assert_eq!(document.to_string(), "hello world");
    assert_eq!(document.get_deltas().len(), 2);
    assert_eq!(document.get_deltas()[1].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[1].operations[0],
        Operation::Insert((5, " world".to_string()))
    );

    document.update("held!");
    assert_eq!(document.to_string(), "held!");
    assert_eq!(document.get_deltas().len(), 3);
    assert_eq!(document.get_deltas()[2].operations.len(), 2);
    assert_eq!(
        document.get_deltas()[2].operations[0],
        Operation::Delete((3, 7))
    );
    assert_eq!(
        document.get_deltas()[2].operations[1],
        Operation::Insert((4, "!".to_string())),
    );
}
