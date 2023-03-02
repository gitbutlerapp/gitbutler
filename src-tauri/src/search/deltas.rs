use crate::{deltas, projects, sessions};
use anyhow::Result;
use std::{fs, path::Path};
use tantivy::{directory::MmapDirectory, schema, DateTime};

#[derive(Clone)]
pub struct DeltasIndex {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
}

fn schema() -> schema::Schema {
    let mut schema_builder = schema::Schema::builder();
    schema_builder.add_date_field(
        "timestamp_ms",
        schema::STORED // we want to retrieve it from search results, so store the value
        | schema::FAST, // makes the field faster to filter / sort on
    );
    schema_builder.add_text_field(
        "session_hash",
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
        schema::FAST // we want to filter on the field
        | schema::INDEXED, // we want to search on the field
    );
    schema_builder.add_u64_field(
        "is_deletion",
        schema::FAST // we want to filter on the field
        | schema::INDEXED, // we want to search on the field
    );
    schema_builder.build()
}

const WRITE_BUFFER_SIZE: usize = 10_000_000; // 10MB

impl DeltasIndex {
    pub fn open_or_create<P: AsRef<Path>>(
        base_path: P,
        project: &projects::Project,
    ) -> Result<Self> {
        let dir = base_path
            .as_ref()
            .join("indexes")
            .join(&project.id)
            .join("deltas");
        fs::create_dir_all(&dir)?;

        let schema = schema();
        let mmap_dir = MmapDirectory::open(dir)?;
        let index = tantivy::Index::open_or_create(mmap_dir, schema)?;
        Ok(Self {
            index: index.clone(),
            reader: index.reader()?,
        })
    }

    fn with_writer(&self, f: impl FnOnce(&tantivy::IndexWriter) -> Result<()>) -> Result<()> {
        let mut writer = self.index.writer(WRITE_BUFFER_SIZE)?;
        println!("Writing...");
        f(&mut writer)?;
        println!("Committing...");
        writer.commit()?;
        Ok(())
    }

    pub fn write(
        &self,
        session: &sessions::Session,
        repo: &git2::Repository,
        project: &projects::Project,
        reference: &git2::Reference,
    ) -> Result<()> {
        let deltas = deltas::list(repo, project, reference, &session.id)?;
        println!("Found {} deltas", deltas.len());
        if deltas.is_empty() {
            return Ok(());
        }
        let files = sessions::list_files(
            repo,
            project,
            reference,
            &session.id,
            Some(deltas.keys().map(|k| k.as_str()).collect()),
        )?;
        match &session.hash {
            None => Err(anyhow::anyhow!("Session hash is not set, on")),
            Some(hash) => self.with_writer(|writer| {
                let field_timestamp_ms = self.index.schema().get_field("timestamp_ms").unwrap();
                let field_session_hash = self.index.schema().get_field("session_hash").unwrap();
                let field_file_path = self.index.schema().get_field("file_path").unwrap();
                let field_diff = self.index.schema().get_field("diff").unwrap();
                let field_is_addition = self.index.schema().get_field("is_addition").unwrap();
                let field_is_deletion = self.index.schema().get_field("is_deletion").unwrap();

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
                    for delta in deltas {
                        // for every operation in the delta
                        for operation in &delta.operations {
                            let mut doc = tantivy::Document::default();
                            doc.add_date(
                                field_timestamp_ms,
                                DateTime::from_timestamp_millis(delta.timestamp_ms.try_into()?),
                            );
                            doc.add_text(field_session_hash, hash);
                            doc.add_text(field_file_path, file_path.as_str());
                            match operation {
                                deltas::Operation::Delete((from, len)) => {
                                    // here we use the file_text to calculate the diff
                                    let diff = file_text
                                        .iter()
                                        .skip((*from).try_into()?)
                                        .take((*len).try_into()?)
                                        .collect::<String>();
                                    doc.add_text(field_diff, diff);
                                    doc.add_bool(field_is_deletion, true);
                                }
                                deltas::Operation::Insert((_from, value)) => {
                                    doc.add_text(field_diff, value);
                                    doc.add_bool(field_is_addition, true);
                                }
                            }
                            writer.add_document(doc)?;

                            // don't forget to apply the operation to the file_text
                            operation.apply(&mut file_text);
                        }
                    }
                }
                Ok(())
            }),
        }
    }
}
