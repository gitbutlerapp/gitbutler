use crate::{deltas, projects, sessions, storage};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time, vec,
};
use tantivy::{collector, directory::MmapDirectory, schema, IndexWriter};

const CURRENT_VERSION: u64 = 1; // should not decrease

#[derive(Clone)]
struct MetaStorage {
    storage: storage::Storage,
}

impl MetaStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            storage: storage::Storage::from_path(base_path),
        }
    }

    pub fn get(&self, project_id: &str, session_hash: &str) -> Result<Option<u64>> {
        let filepath = Path::new("indexes")
            .join("meta")
            .join(project_id)
            .join(session_hash);
        let meta = match self.storage.read(&filepath.to_str().unwrap())? {
            None => None,
            Some(meta) => meta.parse::<u64>().ok(),
        };
        Ok(meta)
    }

    pub fn set(&self, project_id: &str, session_hash: &str, version: u64) -> Result<()> {
        let filepath = Path::new("indexes")
            .join("meta")
            .join(project_id)
            .join(session_hash);
        self.storage
            .write(&filepath.to_str().unwrap(), &version.to_string())?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct Deltas {
    meta_storage: MetaStorage,

    index: tantivy::Index,
    reader: tantivy::IndexReader,
    writer: Arc<Mutex<tantivy::IndexWriter>>,
}

impl Deltas {
    pub fn at(path: PathBuf) -> Result<Self> {
        let dir = path.join("indexes").join("deltas");
        fs::create_dir_all(&dir)?;

        let mmap_dir = MmapDirectory::open(dir)?;
        let schema = build_schema();
        let index_settings = tantivy::IndexSettings {
            ..Default::default()
        };
        let index = tantivy::IndexBuilder::new()
            .schema(schema)
            .settings(index_settings)
            .open_or_create(mmap_dir)?;

        let reader = index.reader()?;
        let writer = index.writer(WRITE_BUFFER_SIZE)?;

        Ok(Self {
            meta_storage: MetaStorage::new(path),
            reader,
            writer: Arc::new(Mutex::new(writer)),
            index,
        })
    }

    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        search(&self.index, &self.reader, query)
    }

    pub fn reindex_project(
        &mut self,
        repo: &git2::Repository,
        project: &projects::Project,
    ) -> Result<()> {
        let start = time::SystemTime::now();

        let reference = repo.find_reference(&project.refname())?;
        let head = repo.find_commit(reference.target().unwrap())?;

        // list all commits from gitbutler head to the first commit
        let mut walker = repo.revwalk()?;
        walker.push(head.id())?;
        walker.set_sorting(git2::Sort::TIME)?;

        for oid in walker {
            let oid = oid?;
            let commit = repo
                .find_commit(oid)
                .with_context(|| format!("Could not find commit {}", oid.to_string()))?;
            let session_id = sessions::id_from_commit(repo, &commit)?;

            let version = self
                .meta_storage
                .get(&project.id, &session_id)?
                .unwrap_or(0);

            if version == CURRENT_VERSION {
                continue;
            }

            let session = sessions::Session::from_commit(repo, &commit).with_context(|| {
                format!("Could not parse commit {} in project", oid.to_string())
            })?;
            self.index_session(repo, project, &session)
                .with_context(|| {
                    format!("Could not index commit {} in project", oid.to_string())
                })?;
        }
        log::info!(
            "Reindexing project {} done, took {}ms",
            project.path,
            time::SystemTime::now().duration_since(start)?.as_millis()
        );
        Ok(())
    }

    pub fn index_session(
        &mut self,
        repo: &git2::Repository,
        project: &projects::Project,
        session: &sessions::Session,
    ) -> Result<()> {
        log::info!("Indexing session {} in {}", session.id, project.path);
        index(
            &self.index,
            &mut self.writer.lock().unwrap(),
            session,
            repo,
            project,
        )?;
        self.meta_storage
            .set(&project.id, &session.id, CURRENT_VERSION)?;
        Ok(())
    }
}

fn build_schema() -> schema::Schema {
    let mut schema_builder = schema::Schema::builder();
    schema_builder.add_u64_field("version", schema::INDEXED | schema::FAST);
    schema_builder.add_text_field("project_id", schema::TEXT | schema::STORED | schema::FAST);
    schema_builder.add_text_field("session_id", schema::STORED);
    schema_builder.add_u64_field("index", schema::STORED);
    schema_builder.add_text_field("file_path", schema::TEXT | schema::STORED | schema::FAST);
    schema_builder.add_text_field("diff", schema::TEXT);
    schema_builder.add_bool_field("is_addition", schema::FAST);
    schema_builder.add_bool_field("is_deletion", schema::FAST);
    schema_builder.add_u64_field("timestamp_ms", schema::FAST);
    schema_builder.build()
}

const WRITE_BUFFER_SIZE: usize = 10_000_000; // 10MB

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub project_id: String,
    pub session_id: String,
    pub file_path: String,
    pub index: u64,
}

fn index(
    index: &tantivy::Index,
    writer: &mut IndexWriter,
    session: &sessions::Session,
    repo: &git2::Repository,
    project: &projects::Project,
) -> Result<()> {
    let reference = repo.find_reference(&project.refname())?;
    let deltas = deltas::list(repo, project, &reference, &session.id)?;
    if deltas.is_empty() {
        return Ok(());
    }
    let files = sessions::list_files(
        repo,
        project,
        &reference,
        &session.id,
        Some(deltas.keys().map(|k| k.as_str()).collect()),
    )?;
    // index every file
    for (file_path, deltas) in deltas.into_iter() {
        // keep the state of the file after each delta operation
        // we need it to calculate diff for delete operations
        let mut file_text: Vec<char> = files
            .get(&file_path)
            .map(|f| f.as_str())
            .unwrap_or("")
            .chars()
            .collect();
        // for every deltas for the file
        for (i, delta) in deltas.into_iter().enumerate() {
            // for every operation in the delta
            for operation in &delta.operations {
                let mut doc = tantivy::Document::default();
                doc.add_u64(
                    index.schema().get_field("version").unwrap(),
                    CURRENT_VERSION.try_into()?,
                );
                doc.add_u64(index.schema().get_field("index").unwrap(), i.try_into()?);
                doc.add_text(
                    index.schema().get_field("session_id").unwrap(),
                    session.id.clone(),
                );
                doc.add_text(
                    index.schema().get_field("file_path").unwrap(),
                    file_path.as_str(),
                );
                doc.add_text(
                    index.schema().get_field("project_id").unwrap(),
                    project.id.clone(),
                );
                doc.add_u64(
                    index.schema().get_field("timestamp_ms").unwrap(),
                    delta.timestamp_ms.try_into()?,
                );
                match operation {
                    deltas::Operation::Delete((from, len)) => {
                        // here we use the file_text to calculate the diff
                        let diff = file_text
                            .iter()
                            .skip((*from).try_into()?)
                            .take((*len).try_into()?)
                            .collect::<String>();
                        doc.add_text(index.schema().get_field("diff").unwrap(), diff);
                        doc.add_bool(index.schema().get_field("is_deletion").unwrap(), true);
                    }
                    deltas::Operation::Insert((_from, value)) => {
                        doc.add_text(index.schema().get_field("diff").unwrap(), value);
                        doc.add_bool(index.schema().get_field("is_addition").unwrap(), true);
                    }
                }
                writer.add_document(doc)?;

                // don't forget to apply the operation to the file_text
                if let Err(e) = operation.apply(&mut file_text) {
                    log::error!("failed to apply operation: {:#}", e);
                    break;
                }
            }
        }
    }
    writer.commit()?;
    Ok(())
}

#[derive(Debug, Default)]
pub struct SearchQuery {
    pub q: String,
    pub project_id: String,
    pub limit: usize,
    pub offset: Option<usize>,
}

pub fn search(
    index: &tantivy::Index,
    reader: &tantivy::IndexReader,
    q: &SearchQuery,
) -> Result<Vec<SearchResult>> {
    let query = &tantivy::query::QueryParser::for_index(
        index,
        vec![
            index.schema().get_field("diff").unwrap(),
            index.schema().get_field("file_path").unwrap(),
        ],
    )
    .parse_query(
        format!(
            "version:\"{}\" AND project_id:\"{}\" AND ({})",
            CURRENT_VERSION, q.project_id, q.q,
        )
        .as_str(),
    )?;

    reader.reload()?;
    let searcher = reader.searcher();

    let top_docs = searcher.search(
        query,
        &collector::TopDocs::with_limit(q.limit)
            .and_offset(q.offset.unwrap_or(0))
            .order_by_u64_field(index.schema().get_field("timestamp_ms").unwrap()),
    )?;

    let results = top_docs
        .iter()
        .map(|(_score, doc_address)| {
            let retrieved_doc = searcher.doc(*doc_address)?;
            let project_id = retrieved_doc
                .get_first(index.schema().get_field("project_id").unwrap())
                .unwrap()
                .as_text()
                .unwrap();
            let file_path = retrieved_doc
                .get_first(index.schema().get_field("file_path").unwrap())
                .unwrap()
                .as_text()
                .unwrap();
            let session_id = retrieved_doc
                .get_first(index.schema().get_field("session_id").unwrap())
                .unwrap()
                .as_text()
                .unwrap();
            let index = retrieved_doc
                .get_first(index.schema().get_field("index").unwrap())
                .unwrap()
                .as_u64()
                .unwrap();
            Ok(SearchResult {
                project_id: project_id.to_string(),
                file_path: file_path.to_string(),
                session_id: session_id.to_string(),
                index,
            })
        })
        .collect::<Result<Vec<SearchResult>>>()?;

    Ok(results)
}
