use crate::{deltas, projects, sessions};
use anyhow::Result;
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
    vec,
};
use tantivy::{collector, directory::MmapDirectory, schema, IndexWriter};

pub struct Deltas {
    base_path: String,

    indexes: HashMap<String, tantivy::Index>,
    readers: HashMap<String, tantivy::IndexReader>,
    writers: HashMap<String, Arc<Mutex<tantivy::IndexWriter>>>,
}

impl Deltas {
    pub fn at<P: AsRef<Path>>(path: P) -> Self {
        Self {
            base_path: path.as_ref().to_str().unwrap().to_string(),
            readers: HashMap::new(),
            writers: HashMap::new(),
            indexes: HashMap::new(),
        }
    }

    fn init(&mut self, project_id: &str) -> Result<()> {
        if self.indexes.contains_key(project_id) {
            return Ok(());
        }

        let index = open_or_create(Path::new(&self.base_path), project_id)?;
        let reader = index.reader()?;
        let writer = index.writer(WRITE_BUFFER_SIZE)?;
        self.readers.insert(project_id.to_string(), reader);
        self.writers
            .insert(project_id.to_string(), Arc::new(Mutex::new(writer)));
        self.indexes.insert(project_id.to_string(), index);
        Ok(())
    }

    pub fn search(&self, project_id: &str, query: &str) -> Result<Vec<SearchResult>> {
        match self.readers.get(project_id) {
            None => Ok(vec![]),
            Some(reader) => {
                let index = self.indexes.get(project_id).unwrap();
                search(index, reader, query)
            }
        }
    }

    pub fn index(
        &mut self,
        repo: &git2::Repository,
        project: &projects::Project,
        session: &sessions::Session,
    ) -> Result<()> {
        self.init(&project.id)?;
        index(
            &self.indexes.get(&project.id).unwrap(),
            &mut self.writers.get(&project.id).unwrap().lock().unwrap(),
            session,
            repo,
            project,
        )
    }
}

fn build_schema() -> schema::Schema {
    let mut schema_builder = schema::Schema::builder();
    schema_builder.add_text_field(
        "session_hash",
        schema::STORED, // store the value so we can retrieve it from search results
    );
    schema_builder.add_u64_field(
        "index",
        schema::STORED, // store the value so we can retrieve it from search results
    );
    schema_builder.add_text_field(
        "file_path",
        schema::TEXT // we want to search on this field, tokenize and index it
        | schema::STORED // store the value so we can retrieve it from search results
        | schema::FAST, // makes the field faster to filter / sort on
    );
    schema_builder.add_text_field(
        "diff",
        schema::TEXT, // we want to search on this field, tokenize and index it
    );
    schema_builder.add_bool_field(
        "is_addition",
        schema::FAST, // we want to filter on the field
    );
    schema_builder.add_u64_field(
        "is_deletion",
        schema::FAST, // we want to filter on the field
    );
    schema_builder.build()
}

const WRITE_BUFFER_SIZE: usize = 10_000_000; // 10MB

pub struct SearchResult {
    pub session_hash: String,
    pub file_path: String,
    pub index: u64,
}

fn open_or_create<P: AsRef<Path>>(base_path: P, project_id: &str) -> Result<tantivy::Index> {
    let dir = base_path
        .as_ref()
        .join("indexes")
        .join(&project_id)
        .join("deltas");
    fs::create_dir_all(&dir)?;

    let mmap_dir = MmapDirectory::open(dir)?;
    let schema = build_schema();
    let index = tantivy::Index::open_or_create(mmap_dir, schema)?;
    Ok(index)
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
    println!("Found {} deltas", deltas.len());
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
    match &session.hash {
        None => Err(anyhow::anyhow!("Session hash is not set, on")),
        Some(hash) => {
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
                        doc.add_u64(index.schema().get_field("index").unwrap(), i.try_into()?);
                        doc.add_text(index.schema().get_field("session_hash").unwrap(), hash);
                        doc.add_text(
                            index.schema().get_field("file_path").unwrap(),
                            file_path.as_str(),
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
                                doc.add_bool(
                                    index.schema().get_field("is_deletion").unwrap(),
                                    true,
                                );
                            }
                            deltas::Operation::Insert((_from, value)) => {
                                doc.add_text(index.schema().get_field("diff").unwrap(), value);
                                doc.add_bool(
                                    index.schema().get_field("is_addition").unwrap(),
                                    true,
                                );
                            }
                        }
                        writer.add_document(doc)?;

                        // don't forget to apply the operation to the file_text
                        operation.apply(&mut file_text);
                    }
                }
            }
            writer.commit()?;
            Ok(())
        }
    }
}

pub fn search(
    index: &tantivy::Index,
    reader: &tantivy::IndexReader,
    q: &str,
) -> Result<Vec<SearchResult>> {
    let query_parser = &tantivy::query::QueryParser::for_index(
        index,
        vec![
            index.schema().get_field("diff").unwrap(),
            index.schema().get_field("file_path").unwrap(),
        ],
    );

    let query = query_parser.parse_query(q)?;

    reader.reload()?;
    let searcher = reader.searcher();
    let top_docs = searcher.search(&query, &collector::TopDocs::with_limit(10))?;

    let results = top_docs
        .iter()
        .map(|(_score, doc_address)| {
            let retrieved_doc = searcher.doc(*doc_address)?;
            let file_path = retrieved_doc
                .get_first(index.schema().get_field("file_path").unwrap())
                .unwrap()
                .as_text()
                .unwrap();
            let session_hash = retrieved_doc
                .get_first(index.schema().get_field("session_hash").unwrap())
                .unwrap()
                .as_text()
                .unwrap();
            let index = retrieved_doc
                .get_first(index.schema().get_field("index").unwrap())
                .unwrap()
                .as_u64()
                .unwrap();
            Ok(SearchResult {
                file_path: file_path.to_string(),
                session_hash: session_hash.to_string(),
                index,
            })
        })
        .collect::<Result<Vec<SearchResult>>>()?;

    Ok(results)
}
