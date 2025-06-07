//! This is due to be replaced with a real vector database. I just got bored
//! with trying to get them to work.
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher as _},
    path::PathBuf,
};

use anyhow::{Result, bail};
use bstr::BString;
use gitbutler_command_context::CommandContext;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Serialize, Deserialize, Clone)]
pub struct Hunk {
    /// The sha of the commit
    #[serde(with = "gitbutler_serde::object_id")]
    pub oid: gix::ObjectId,
    /// Header
    pub header: String,
    pub path: BString,
    pub previous_path: Option<BString>,
    pub vector: Vec<f32>,
}

impl Hunk {
    // We should only ever have one entry per commit & hunk hearder
    pub fn key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.oid, &self.header).hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Serialize, Deserialize, Clone)]
/// Used to denote a commit has been processed
pub struct Commit {
    /// The sha of the commit
    #[serde(with = "gitbutler_serde::object_id")]
    pub oid: gix::ObjectId,
}

#[derive(Serialize, Deserialize)]
pub struct Db {
    pub hunks: Vec<Hunk>,
    pub commits: Vec<Commit>,
}

pub struct DbHandle {
    path: PathBuf,
}

// TODO: Replace with real vector database
impl DbHandle {
    pub fn new(ctx: &CommandContext) -> Self {
        Self {
            path: ctx.project().gb_dir().join("inspection.json"),
        }
    }

    #[instrument(skip_all)]
    pub fn read(&self) -> Result<Db> {
        if std::fs::exists(&self.path)? {
            let content = std::fs::read_to_string(&self.path)?;
            let content: Db = serde_json::from_str(&content)?;
            Ok(content)
        } else {
            Ok(Db {
                hunks: vec![],
                commits: vec![],
            })
        }
    }

    #[instrument(skip_all)]
    fn write(&self, db: &Db) -> Result<()> {
        let content = serde_json::to_string(db)?;
        std::fs::create_dir_all(self.path.parent().unwrap())?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    #[instrument(skip_all)]
    pub fn upsert_many_hunks(&self, entries: &[Hunk]) -> Result<Vec<Hunk>> {
        let mut db = self.read()?;
        let mut map = db
            .hunks
            .into_iter()
            .map(|e| (e.key(), e))
            .collect::<HashMap<u64, Hunk>>();

        for e in entries {
            map.insert(e.key(), e.clone());
        }

        db.hunks = map.into_values().collect::<Vec<_>>();

        self.upsert_many_commits(
            &entries
                .iter()
                .map(|h| Commit { oid: h.oid })
                .collect::<Vec<Commit>>(),
        )?;

        self.write(&db)?;

        Ok(db.hunks)
    }

    #[instrument(skip_all)]
    pub fn upsert_many_commits(&self, entries: &[Commit]) -> Result<Vec<Commit>> {
        let mut db = self.read()?;
        let mut map = db
            .commits
            .into_iter()
            .map(|e| (e.oid, e))
            .collect::<HashMap<gix::ObjectId, Commit>>();

        for e in entries {
            map.insert(e.oid, e.clone());
        }

        db.commits = map.into_values().collect::<Vec<_>>();

        self.write(&db)?;

        Ok(db.commits)
    }

    // TODO: Replace with real vector db search rather than a manual implementation.
    #[instrument(skip_all)]
    pub fn search_hunks(&self, term: Vec<f32>, cutoff: Option<usize>) -> Result<Vec<(Hunk, f32)>> {
        let db = self.read()?;

        let mut with_distance = db
            .hunks
            .into_iter()
            .map(|i| {
                let distance = cosine_distance(&i.vector, &term)?;
                Ok((i, distance))
            })
            .collect::<Result<Vec<(Hunk, f32)>>>()?;

        // Sort decending
        with_distance.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        if let Some(cutoff) = cutoff {
            Ok(with_distance.into_iter().take(cutoff).collect())
        } else {
            Ok(with_distance)
        }
    }
}

fn cosine_distance(a: &[f32], b: &[f32]) -> Result<f32> {
    if a.len() != b.len() {
        bail!("Vectors MUST be the same length!")
    };

    let dot: f32 = a.iter().zip(b).map(|(a, b)| a * b).sum();
    let am: f32 = a.iter().fold(0.0, |acc, a| acc + (a * a)).powf(0.5);
    let bm: f32 = b.iter().fold(0.0, |acc, a| acc + (a * a)).powf(0.5);
    Ok(dot / (am * bm))
}
