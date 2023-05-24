use std::{
    fs,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use tantivy::{collector, directory::MmapDirectory, schema};

use crate::bookmarks;

pub struct Bookmarks {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
    writer: Arc<Mutex<tantivy::IndexWriter>>,
}

const CURRENT_VERSION: u64 = 0; // should not decrease
const WRITE_BUFFER_SIZE: usize = 10_000_000; // 10MB

fn build_schema() -> schema::Schema {
    let mut schema_builder = schema::Schema::builder();
    schema_builder.add_u64_field("version", schema::INDEXED);
    schema_builder.add_text_field("id", schema::STRING);
    schema_builder.add_text_field("project_id", schema::STRING);
    schema_builder.add_u64_field("timestamp_ms", schema::INDEXED | schema::STORED);
    schema_builder.add_text_field("note", schema::TEXT);
    schema_builder.build()
}

fn build_id(bookmark: &bookmarks::Bookmark) -> String {
    format!(
        "{}-{}-{}",
        CURRENT_VERSION, bookmark.project_id, bookmark.timestamp_ms
    )
}

#[derive(Debug)]
pub struct SearchQuery {
    pub q: String,
    pub project_id: String,
    pub limit: usize,
    pub offset: Option<usize>,
}

impl Bookmarks {
    pub fn at<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let dir = path
            .join("indexes")
            .join(format!("v{}", CURRENT_VERSION))
            .join("bookmarks");
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
        let writer = index.writer_with_num_threads(1, WRITE_BUFFER_SIZE)?;

        Ok(Self {
            reader,
            writer: Arc::new(Mutex::new(writer)),
            index,
        })
    }

    pub fn index(&self, bookmark: &bookmarks::Bookmark) -> Result<()> {
        let mut doc = tantivy::Document::default();
        doc.add_u64(
            self.index.schema().get_field("version").unwrap(),
            CURRENT_VERSION.try_into()?,
        );
        doc.add_u64(
            self.index.schema().get_field("timestamp_ms").unwrap(),
            bookmark.timestamp_ms.try_into()?,
        );
        doc.add_text(
            self.index.schema().get_field("note").unwrap(),
            bookmark.note.clone(),
        );
        doc.add_text(
            self.index.schema().get_field("project_id").unwrap(),
            bookmark.project_id.clone(),
        );
        doc.add_text(
            self.index.schema().get_field("id").unwrap(),
            build_id(bookmark),
        );

        let mut writer = self.writer.lock().unwrap();
        writer.delete_term(tantivy::Term::from_field_text(
            self.index.schema().get_field("id").unwrap(),
            &build_id(bookmark),
        ));
        writer.add_document(doc)?;
        writer.commit()?;
        Ok(())
    }

    pub fn search(&self, q: &SearchQuery) -> Result<Vec<u128>> {
        self.reader.reload()?;
        let searcher = self.reader.searcher();
        let schema = self.index.schema();
        let version_field = schema.get_field("version").unwrap();
        let project_id_field = schema.get_field("project_id").unwrap();
        let timestamp_ms_field = schema.get_field("timestamp_ms").unwrap();
        let note_field = schema.get_field("note").unwrap();

        let top_docs = searcher.search(
            &tantivy::query::BooleanQuery::new_multiterms_query(vec![
                tantivy::Term::from_field_u64(version_field, CURRENT_VERSION),
                tantivy::Term::from_field_text(project_id_field, q.project_id.as_str()),
                tantivy::Term::from_field_text(note_field, q.q.as_str()),
            ]),
            &collector::TopDocs::with_limit(q.limit).and_offset(q.offset.unwrap_or(0)),
        )?;

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let doc = searcher.doc(doc_address)?;
            let timestamp_ms: u128 = doc
                .get_first(timestamp_ms_field)
                .unwrap()
                .as_u64()
                .unwrap()
                .try_into()?;
            results.push(timestamp_ms)
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn index_and_retrieve_single() -> Result<()> {
        let dir = tempdir().unwrap();
        let path = PathBuf::from(dir.path());
        let bookmarks = Bookmarks::at(path)?;

        let bookmark = bookmarks::Bookmark {
            project_id: "test".to_string(),
            timestamp_ms: 0,
            note: "hello world".to_string(),
            created_timestamp_ms: 0,
            updated_timestamp_ms: 0,
            deleted: false,
        };
        bookmarks.index(&bookmark).unwrap();

        let query = SearchQuery {
            q: "hello".to_string(),
            project_id: bookmark.project_id.clone(),
            limit: 10,
            offset: None,
        };

        let results = bookmarks.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], bookmark.timestamp_ms);

        Ok(())
    }
}
