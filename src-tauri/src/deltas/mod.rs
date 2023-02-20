mod deltas;
mod operations;
mod text_document;

pub use deltas::{list, read, write, Delta};
pub use text_document::TextDocument;

#[cfg(test)]
mod deltas_tests;
#[cfg(test)]
mod text_document_tests;
