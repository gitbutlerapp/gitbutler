use std::{
    fs, path,
    sync::{Arc, RwLock},
    time, vec,
};

use anyhow::{Context, Result};
use serde::Serialize;
use similar::{ChangeTag, TextDiff};
use std::cmp::Ordering;
use tantivy::query::TermQuery;
use tantivy::{collector, directory::MmapDirectory, IndexWriter};
use tantivy::{query::QueryParser, Term};
use tantivy::{schema::IndexRecordOption, tokenizer};
use tauri::AppHandle;

use crate::{bookmarks, deltas, gb_repository, paths::DataDir, reader, sessions};

use super::{index, meta};

#[derive(Clone)]
pub struct Searcher {
    inner: Arc<SearcherInner>,
}

impl TryFrom<&AppHandle> for Searcher {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(SearcherInner::try_from(value)?),
        })
    }
}

impl TryFrom<&DataDir> for Searcher {
    type Error = anyhow::Error;

    fn try_from(value: &DataDir) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(SearcherInner::try_from(value)?),
        })
    }
}

impl Searcher {
    pub fn search(&self, q: &Query) -> Result<Results> {
        self.inner.search(q)
    }

    pub fn delete_all_data(&self) -> Result<()> {
        self.inner.delete_all_data()
    }

    pub fn index_bookmark(
        &self,
        bookmark: &bookmarks::Bookmark,
    ) -> Result<Option<index::IndexDocument>> {
        self.inner.index_bookmark(bookmark)
    }

    pub fn index_session(
        &self,
        repository: &gb_repository::Repository,
        session: &sessions::Session,
    ) -> Result<()> {
        self.inner.index_session(repository, session)
    }
}

pub struct SearcherInner {
    meta_storage: meta::Storage,

    index: tantivy::Index,
    reader: tantivy::IndexReader,
    writer: Arc<RwLock<tantivy::IndexWriter>>,
}

impl TryFrom<&AppHandle> for SearcherInner {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self> {
        let data_dir = DataDir::try_from(value)?;
        Self::open(&data_dir)
    }
}

impl TryFrom<&DataDir> for SearcherInner {
    type Error = anyhow::Error;

    fn try_from(path: &DataDir) -> Result<Self> {
        Self::open(path)
    }
}

impl SearcherInner {
    fn open(value: &DataDir) -> Result<Self> {
        let dir = value
            .to_path_buf()
            .join("indexes")
            .join(format!("v{}", index::VERSION))
            .join("deltas");
        fs::create_dir_all(&dir)?;

        let mmap_dir = MmapDirectory::open(dir)?;
        let index_settings = tantivy::IndexSettings {
            ..Default::default()
        };
        let index = tantivy::IndexBuilder::new()
            .schema(index::build_schema())
            .settings(index_settings)
            .open_or_create(mmap_dir)?;

        index.tokenizers().register(
            "ngram2_3",
            tokenizer::TextAnalyzer::builder(tokenizer::NgramTokenizer::all_ngrams(2, 3))
                .filter(tokenizer::LowerCaser)
                .build(),
        );

        let reader = index.reader()?;
        let writer = index.writer_with_num_threads(1, WRITE_BUFFER_SIZE)?;

        Ok(Self {
            meta_storage: meta::Storage::from(value),
            reader,
            writer: Arc::new(RwLock::new(writer)),
            index,
        })
    }

    pub fn search(&self, q: &Query) -> Result<Results> {
        let version_field = self.index.schema().get_field("version").unwrap();
        let project_id_field = self.index.schema().get_field("project_id").unwrap();
        let diff_field = self.index.schema().get_field("diff").unwrap();
        let file_path_field = self.index.schema().get_field("file_path").unwrap();
        let note_field = self.index.schema().get_field("note").unwrap();

        let version_term_query = Box::new(TermQuery::new(
            Term::from_field_u64(version_field, index::VERSION),
            IndexRecordOption::Basic,
        ));
        let project_id_term_query = Box::new(TermQuery::new(
            Term::from_field_text(project_id_field, q.project_id.as_str()),
            IndexRecordOption::Basic,
        ));

        let diff_or_file_path_or_note_query = Box::new({
            let parser =
                QueryParser::for_index(&self.index, vec![diff_field, file_path_field, note_field]);
            parser.parse_query(&format!("\"{}\"", &q.q))?
        });

        let query = tantivy::query::BooleanQuery::intersection(vec![
            version_term_query,
            project_id_term_query,
            diff_or_file_path_or_note_query,
        ]);

        let searcher = self.reader.searcher();

        let mut collectors = collector::MultiCollector::new();
        let top_docs_handle = collectors.add_collector(
            collector::TopDocs::with_limit(q.limit)
                .and_offset(q.offset.unwrap_or(0))
                .order_by_u64_field("timestamp_ms"),
        );
        let count_handle = collectors.add_collector(collector::Count);

        let mut result = searcher.search(&query, &collectors)?;
        let count = count_handle.extract(&mut result);
        let top_docs = top_docs_handle.extract(&mut result);

        let page = top_docs
            .iter()
            .map(|(_score, doc_address)| {
                let doc = &searcher.doc(*doc_address)?;
                let index_document = index::IndexDocument::from_document(&self.index.schema(), doc);
                Ok(SearchResult {
                    project_id: index_document.project_id.unwrap(),
                    file_path: index_document.file_path.unwrap(),
                    session_id: index_document.session_id.unwrap(),
                    index: index_document.index.unwrap(),
                })
            })
            .collect::<Result<Vec<SearchResult>>>()?;

        Ok(Results { page, total: count })
    }

    pub fn delete_all_data(&self) -> Result<()> {
        self.meta_storage
            .delete_all()
            .context("Could not delete meta data")?;
        let mut writer = self.writer.write().unwrap();
        writer
            .delete_all_documents()
            .context("Could not delete all documents")?;
        writer.commit().context("Could not commit")?;
        self.reader.reload()?;
        Ok(())
    }

    pub fn index_bookmark(
        &self,
        bookmark: &bookmarks::Bookmark,
    ) -> Result<Option<index::IndexDocument>> {
        let id = build_id(&bookmark.project_id, &bookmark.timestamp_ms);
        let id_field = self.index.schema().get_field("id").unwrap();

        let mut writer = self.writer.write().unwrap();
        let mut doc = match find_document_by_id(&self.index, &self.reader, &id)? {
            Some(doc) => {
                if doc
                    .indexed_at
                    .cmp(&bookmark.updated_timestamp_ms.try_into()?)
                    == Ordering::Greater
                {
                    return Ok(None);
                }
                doc
            }
            None => index::IndexDocument {
                id: id.clone(),
                version: index::VERSION,
                indexed_at: time::UNIX_EPOCH.elapsed()?.as_millis().try_into()?,
                ..Default::default()
            },
        };

        let note = if bookmark.deleted { "" } else { &bookmark.note };
        if note == doc.note.clone().unwrap_or_default() {
            return Ok(None);
        }

        doc.note = Some(note.to_string());

        writer.delete_term(Term::from_field_text(id_field, id.as_str()));
        writer.add_document(doc.to_document(&self.index.schema()))?;
        writer.commit()?;
        self.reader.reload()?;

        tracing::debug!(
            project_id = bookmark.project_id,
            timestamp_ms = bookmark.timestamp_ms,
            "bookmark added to search",
        );

        Ok(Some(doc))
    }

    pub fn index_session(
        &self,
        repository: &gb_repository::Repository,
        session: &sessions::Session,
    ) -> Result<()> {
        // TODO: maybe we should index current sessions?
        if session.hash.is_none() {
            return Ok(());
        }

        let version = self
            .meta_storage
            .get(repository.get_project_id(), &session.id)?
            .unwrap_or(0);

        if version == index::VERSION {
            return Ok(());
        }

        index_session(
            &self.index,
            &mut self.writer.write().unwrap(),
            &self.reader,
            session,
            repository,
        )?;
        self.meta_storage
            .set(repository.get_project_id(), &session.id, index::VERSION)?;

        tracing::debug!(
            project_id = repository.get_project_id(),
            session_id = session.id,
            "session added to search",
        );

        Ok(())
    }
}

fn build_id(project_id: &str, timestamp_ms: &u128) -> String {
    format!("{}-{}-{}", index::VERSION, project_id, timestamp_ms)
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Results {
    pub page: Vec<SearchResult>,
    pub total: usize,
}

fn index_session(
    index: &tantivy::Index,
    writer: &mut IndexWriter,
    reader: &tantivy::IndexReader,
    session: &sessions::Session,
    repository: &gb_repository::Repository,
) -> Result<()> {
    let session_reader = sessions::Reader::open(repository, session)
        .with_context(|| "could not get session reader")?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader
        .read(None)
        .with_context(|| "could not list deltas for session")?;
    if deltas.is_empty() {
        return Ok(());
    }
    let files = session_reader
        .files(Some(deltas.keys().map(|s| s.to_path_buf()).collect()))
        .context("could not list files for session")?;
    // index every file
    for (file_path, deltas) in deltas.into_iter() {
        // keep the state of the file after each delta operation
        // we need it to calculate diff for delete operations
        let mut file_text: Vec<char> = files
            .get(&file_path)
            .map(|content| match content {
                reader::Content::UTF8(text) => text.chars().collect(),
                _ => vec![],
            })
            .unwrap_or_default();

        // for every deltas for the file
        for (i, delta) in deltas.into_iter().enumerate() {
            index_delta(
                index,
                writer,
                reader,
                &session.id,
                repository.get_project_id(),
                &mut file_text,
                &file_path,
                i,
                &delta,
            )?;
        }
    }
    writer.commit()?;
    reader.reload()?;
    Ok(())
}

fn find_document_by_id(
    index: &tantivy::Index,
    reader: &tantivy::IndexReader,
    id: &str,
) -> Result<Option<index::IndexDocument>> {
    let id_field = index.schema().get_field("id").unwrap();
    let searcher = reader.searcher();
    let query = TermQuery::new(
        Term::from_field_text(id_field, id),
        tantivy::schema::IndexRecordOption::Basic,
    );
    let top_docs = searcher.search(&query, &collector::TopDocs::with_limit(1))?;
    if top_docs.is_empty() {
        return Ok(None);
    }
    let doc_address = top_docs[0].1;
    let doc = searcher.doc(doc_address)?;
    Ok(Some(index::IndexDocument::from_document(
        &index.schema(),
        &doc,
    )))
}

#[allow(clippy::too_many_arguments)]
fn index_delta(
    index: &tantivy::Index,
    writer: &mut IndexWriter,
    reader: &tantivy::IndexReader,
    session_id: &str,
    project_id: &str,
    file_text: &mut Vec<char>,
    file_path: &path::Path,
    i: usize,
    delta: &deltas::Delta,
) -> Result<()> {
    let id = build_id(project_id, &delta.timestamp_ms);
    let id_field = index.schema().get_field("id").unwrap();

    let mut doc = match find_document_by_id(index, reader, &id)? {
        Some(doc) => {
            writer.delete_term(Term::from_field_text(id_field, id.as_str()));
            doc
        }
        None => index::IndexDocument {
            id: id.clone(),
            version: index::VERSION,
            indexed_at: time::UNIX_EPOCH.elapsed()?.as_millis().try_into()?,
            ..Default::default()
        },
    };

    let prev_file_text = file_text.clone();
    // for every operation in the delta
    for operation in &delta.operations {
        // don't forget to apply the operation to the file_text
        operation.apply(file_text).context(format!(
            "Could not apply operation to file {}",
            file_path.display()
        ))?;
    }

    let old = &prev_file_text.iter().collect::<String>();
    let new = &file_text.iter().collect::<String>();

    let all_changes = TextDiff::from_words(old, new);
    let changes = all_changes
        .iter_all_changes()
        .filter_map(|change| match change.tag() {
            ChangeTag::Delete => change.as_str(),
            ChangeTag::Insert => change.as_str(),
            ChangeTag::Equal => None,
        })
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join(" ");

    doc.index = Some(i.try_into()?);
    doc.session_id = Some(session_id.to_string());
    doc.file_path = Some(file_path.display().to_string());
    doc.project_id = Some(project_id.to_string());
    doc.timestamp_ms = Some(delta.timestamp_ms.try_into()?);
    doc.diff = Some(changes);

    writer.add_document(doc.to_document(&index.schema()))?;

    Ok(())
}

#[derive(Debug)]
pub struct Query {
    pub q: String,
    pub project_id: String,
    pub limit: usize,
    pub offset: Option<usize>,
}
