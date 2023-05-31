use tantivy::{
    schema::{self, TextFieldIndexing, TextOptions},
    Document,
};

#[derive(Debug, Default)]
pub struct IndexDocument {
    pub version: u64,
    pub timestamp_ms: Option<u64>,
    pub index: Option<u64>,
    pub id: String,
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub file_path: Option<String>,
    pub diff: Option<String>,
    pub note: Option<String>,
}

impl IndexDocument {
    pub fn to_document(&self, schema: &schema::Schema) -> Document {
        let mut doc = Document::default();
        doc.add_u64(schema.get_field("version").unwrap(), self.version);
        if let Some(timestamp_ms) = self.timestamp_ms {
            doc.add_u64(schema.get_field("timestamp_ms").unwrap(), timestamp_ms);
        }
        if let Some(index) = self.index {
            doc.add_u64(schema.get_field("index").unwrap(), index);
        }
        doc.add_text(schema.get_field("id").unwrap(), &self.id);
        if let Some(project_id) = self.project_id.as_ref() {
            doc.add_text(schema.get_field("project_id").unwrap(), project_id);
        }
        if let Some(session_id) = self.session_id.as_ref() {
            doc.add_text(schema.get_field("session_id").unwrap(), session_id);
        }
        if let Some(file_path) = self.file_path.as_ref() {
            doc.add_text(schema.get_field("file_path").unwrap(), file_path);
        }
        if let Some(diff) = self.diff.as_ref() {
            doc.add_text(schema.get_field("diff").unwrap(), diff);
        }
        if let Some(note) = self.note.as_ref() {
            doc.add_text(schema.get_field("note").unwrap(), note);
        }
        doc
    }

    pub fn from_document(schema: &schema::Schema, doc: &Document) -> Self {
        let version = doc
            .get_first(schema.get_field("version").unwrap())
            .unwrap()
            .as_u64()
            .unwrap();
        let timestamp_ms = doc
            .get_first(schema.get_field("timestamp_ms").unwrap())
            .map(|v| v.as_u64().unwrap());
        let index = doc
            .get_first(schema.get_field("index").unwrap())
            .map(|v| v.as_u64().unwrap());
        let id = doc
            .get_first(schema.get_field("id").unwrap())
            .unwrap()
            .as_text()
            .unwrap()
            .to_string();
        let project_id = doc
            .get_first(schema.get_field("project_id").unwrap())
            .map(|v| v.as_text().unwrap().to_string());
        let session_id = doc
            .get_first(schema.get_field("session_id").unwrap())
            .map(|v| v.as_text().unwrap().to_string());
        let file_path = doc
            .get_first(schema.get_field("file_path").unwrap())
            .map(|v| v.as_text().unwrap().to_string());
        let diff = doc
            .get_first(schema.get_field("diff").unwrap())
            .map(|v| v.as_text().unwrap().to_string());
        let note = doc
            .get_first(schema.get_field("note").unwrap())
            .map(|v| v.as_text().unwrap().to_string());
        Self {
            version,
            timestamp_ms,
            index,
            id,
            project_id,
            session_id,
            file_path,
            diff,
            note,
        }
    }
}

pub const VERSION: u64 = 5; // should not decrease

pub fn build_schema() -> schema::Schema {
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
        .set_stored(); // text values stored to allow updating document

    let code_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("ngram2_3") // text is indexed with ngram tokenizer to allow partial matching
                .set_index_option(schema::IndexRecordOption::WithFreqsAndPositions), // text is indexed with positions to allow highlighted snippets generation
        )
        .set_stored(); // text values stored to allow updating document

    schema_builder.add_text_field("file_path", text_options.clone());
    schema_builder.add_text_field("diff", code_options);
    schema_builder.add_text_field("note", text_options);

    schema_builder.build()
}
