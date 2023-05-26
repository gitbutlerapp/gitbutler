use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    vec,
};

use anyhow::{Context, Result};
use serde::Serialize;
use similar::{ChangeTag, TextDiff};
use tantivy::{collector, directory::MmapDirectory, schema, Document, IndexWriter};
use tantivy::{query::QueryParser, Term};
use tantivy::{
    query::TermQuery,
    schema::{TextFieldIndexing, TextOptions},
};
use tantivy::{schema::IndexRecordOption, tokenizer};

use crate::{bookmarks, deltas, gb_repository, sessions, storage};

const CURRENT_VERSION: u64 = 5; // should not decrease

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

    pub fn delete_all(&self) -> Result<()> {
        let filepath = self
            .storage
            .local_data_dir()
            .join("indexes")
            .join(format!("v{}", CURRENT_VERSION))
            .join("meta");
        fs::remove_dir_all(filepath)?;
        Ok(())
    }

    pub fn get(&self, project_id: &str, session_hash: &str) -> Result<Option<u64>> {
        let filepath = Path::new("indexes")
            .join(format!("v{}", CURRENT_VERSION))
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
            .join(format!("v{}", CURRENT_VERSION))
            .join("meta")
            .join(project_id)
            .join(session_hash);
        self.storage
            .write(&filepath.to_str().unwrap(), &version.to_string())?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct Searcher {
    meta_storage: MetaStorage,

    index: tantivy::Index,
    reader: tantivy::IndexReader,
    writer: Arc<Mutex<tantivy::IndexWriter>>,
}

impl Searcher {
    pub fn at<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let dir = path
            .join("indexes")
            .join(format!("v{}", CURRENT_VERSION))
            .join("deltas");
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

        index.tokenizers().register(
            "ngram2_3",
            tokenizer::TextAnalyzer::from(tokenizer::NgramTokenizer::all_ngrams(2, 3))
                .filter(tokenizer::LowerCaser),
        );

        let reader = index.reader()?;
        let writer = index.writer_with_num_threads(1, WRITE_BUFFER_SIZE)?;

        Ok(Self {
            meta_storage: MetaStorage::new(path),
            reader,
            writer: Arc::new(Mutex::new(writer)),
            index,
        })
    }

    pub fn search(&self, q: &Query) -> Result<Results> {
        let version_field = self.index.schema().get_field("version").unwrap();
        let project_id_field = self.index.schema().get_field("project_id").unwrap();
        let diff_field = self.index.schema().get_field("diff").unwrap();
        let file_path_field = self.index.schema().get_field("file_path").unwrap();
        let timestamp_ns_field = self.index.schema().get_field("timestamp_ms").unwrap();
        let note_field = self.index.schema().get_field("note").unwrap();

        let version_term_query = Box::new(TermQuery::new(
            Term::from_field_u64(version_field, CURRENT_VERSION),
            IndexRecordOption::Basic,
        ));
        let project_id_term_query = Box::new(TermQuery::new(
            Term::from_field_text(project_id_field, q.project_id.as_str()),
            IndexRecordOption::Basic,
        ));

        let diff_or_file_path_or_note_query = Box::new({
            let mut parser =
                QueryParser::for_index(&self.index, vec![diff_field, file_path_field, note_field]);
            parser.set_conjunction_by_default();
            parser.parse_query(&q.q)?
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
                .order_by_u64_field(timestamp_ns_field),
        );
        let count_handle = collectors.add_collector(collector::Count);

        let snippet_generator = tantivy::SnippetGenerator::create(
            &searcher,
            &query,
            self.index.schema().get_field("diff").unwrap(),
        )?;

        let mut result = searcher.search(&query, &collectors)?;
        let count = count_handle.extract(&mut result);
        let top_docs = top_docs_handle.extract(&mut result);

        let page = top_docs
            .iter()
            .map(|(_score, doc_address)| {
                let retrieved_doc = searcher.doc(*doc_address)?;

                let project_id = retrieved_doc
                    .get_first(self.index.schema().get_field("project_id").unwrap())
                    .unwrap()
                    .as_text()
                    .unwrap();
                let file_path = retrieved_doc
                    .get_first(self.index.schema().get_field("file_path").unwrap())
                    .unwrap()
                    .as_text()
                    .unwrap();
                let session_id = retrieved_doc
                    .get_first(self.index.schema().get_field("session_id").unwrap())
                    .unwrap()
                    .as_text()
                    .unwrap();
                let index = retrieved_doc
                    .get_first(self.index.schema().get_field("index").unwrap())
                    .unwrap()
                    .as_u64()
                    .unwrap();
                let snippet = snippet_generator.snippet_from_doc(&retrieved_doc);
                let fragment = snippet.fragment();
                let highlighted: Vec<String> = snippet
                    .highlighted()
                    .iter()
                    .map(|range| fragment[range.start..range.end].to_string())
                    .collect();
                Ok(SearchResult {
                    project_id: project_id.to_string(),
                    file_path: file_path.to_string(),
                    session_id: session_id.to_string(),
                    highlighted,
                    index,
                })
            })
            .collect::<Result<Vec<SearchResult>>>()?;

        Ok(Results { page, total: count })
    }

    pub fn delete_all_data(&self) -> Result<()> {
        self.meta_storage
            .delete_all()
            .context("Could not delete meta data")?;
        let mut writer = self.writer.lock().unwrap();
        writer
            .delete_all_documents()
            .context("Could not delete all documents")?;
        writer.commit().context("Could not commit")?;
        self.reader.reload()?;
        Ok(())
    }

    pub fn index_bookmark(&self, bookmark: &bookmarks::Bookmark) -> Result<()> {
        let id = build_id(&bookmark.project_id, &bookmark.timestamp_ms);
        let id_field = self.index.schema().get_field("id").unwrap();

        let mut writer = self.writer.lock().unwrap();
        let mut doc = match find_document_by_id(&self.index, &self.reader, &id)? {
            Some(doc) => {
                writer.delete_term(Term::from_field_text(id_field, id.as_str()));
                doc
            }
            None => Document::default(),
        };

        doc.add_text(id_field, &id);
        doc.add_text(
            self.index.schema().get_field("note").unwrap(),
            if bookmark.deleted {
                ""
            } else {
                bookmark.note.as_str()
            },
        );

        writer.add_document(doc)?;
        writer.commit()?;
        self.reader.reload()?;

        Ok(())
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

        if version == CURRENT_VERSION {
            return Ok(());
        }

        index_session(
            &self.index,
            &mut self.writer.lock().unwrap(),
            &self.reader,
            &session,
            &repository,
        )?;
        self.meta_storage
            .set(&repository.get_project_id(), &session.id, CURRENT_VERSION)?;

        log::info!(
            "{}: indexed session {}",
            repository.get_project_id(),
            session.id,
        );

        Ok(())
    }
}

fn build_id(project_id: &str, timestamp_ms: &u128) -> String {
    format!("{}-{}-{}", CURRENT_VERSION, project_id, timestamp_ms)
}

fn build_schema() -> schema::Schema {
    let mut schema_builder = schema::Schema::builder();

    schema_builder.add_u64_field(
        "version",
        schema::INDEXED // version is searchable to allow reindexing
        | schema::STORED, // version is stored to allow updating document
    );
    schema_builder.add_u64_field(
        "timestamp_ms",
        schema::STORED // timestamp is stored to allow updating document
        |schema::FAST, // timestamp is fast to allow sorting
    );
    schema_builder.add_u64_field(
        "index",
        schema::STORED, // index is stored because we want to return it in search results and allow
                        // filtering
    );

    let id_options = TextOptions::default()
        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("raw")) // id is indexed raw to allow exact matching only
        .set_stored(); // and stored to allow updates document

    schema_builder.add_text_field("id", id_options.clone());
    schema_builder.add_text_field("project_id", id_options.clone());
    schema_builder.add_text_field("session_id", id_options);

    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("ngram2_3") // text is indexed with ngram tokenizer to allow partial matching
                .set_index_option(schema::IndexRecordOption::WithFreqsAndPositions), // text is indexed with positions to allow highlighted snippets generation
        )
        .set_stored(); // text values stored to aloow updating document

    schema_builder.add_text_field("file_path", text_options.clone());
    schema_builder.add_text_field("diff", text_options.clone());
    schema_builder.add_text_field("note", text_options);

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
    pub highlighted: Vec<String>,
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
    let session_reader = sessions::Reader::open(&repository, &session)
        .with_context(|| "could not get session reader")?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader
        .read(None)
        .with_context(|| "could not list deltas for session")?;
    if deltas.is_empty() {
        return Ok(());
    }
    let files = session_reader
        .files(Some(deltas.keys().map(|k| k.as_str()).collect()))
        .with_context(|| "could not list files for session")?;
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
            index_delta(
                index,
                writer,
                reader,
                &session.id,
                &repository.get_project_id(),
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
) -> Result<Option<Document>> {
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
    Ok(Some(doc))
}

fn index_delta(
    index: &tantivy::Index,
    writer: &mut IndexWriter,
    reader: &tantivy::IndexReader,
    session_id: &str,
    project_id: &str,
    file_text: &mut Vec<char>,
    file_path: &str,
    i: usize,
    delta: &deltas::Delta,
) -> Result<()> {
    let id = build_id(project_id, &delta.timestamp_ms);
    let id_field = index.schema().get_field("id").unwrap();

    let mut doc = match find_document_by_id(&index, &reader, &id)? {
        Some(doc) => {
            writer.delete_term(Term::from_field_text(id_field, id.as_str()));
            doc
        }
        None => Document::default(),
    };

    doc.add_text(id_field, id);
    doc.add_u64(
        index.schema().get_field("version").unwrap(),
        CURRENT_VERSION.try_into()?,
    );
    doc.add_u64(index.schema().get_field("index").unwrap(), i.try_into()?);
    doc.add_text(
        index.schema().get_field("session_id").unwrap(),
        session_id.clone(),
    );
    doc.add_text(index.schema().get_field("file_path").unwrap(), file_path);
    doc.add_text(index.schema().get_field("project_id").unwrap(), project_id);
    doc.add_u64(
        index.schema().get_field("timestamp_ms").unwrap(),
        delta.timestamp_ms.try_into()?,
    );

    let prev_file_text = file_text.clone();
    // for every operation in the delta
    for operation in &delta.operations {
        // don't forget to apply the operation to the file_text
        operation
            .apply(file_text)
            .with_context(|| format!("Could not apply operation to file {}", file_path))?;
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

    doc.add_text(index.schema().get_field("diff").unwrap(), changes);

    writer.add_document(doc)?;

    Ok(())
}

#[derive(Debug)]
pub struct Query {
    pub q: String,
    pub project_id: String,
    pub limit: usize,
    pub offset: Option<usize>,
}
