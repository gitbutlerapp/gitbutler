mod deltas;
mod bookmarks;

pub use deltas::{Deltas, SearchQuery, SearchResults};

#[cfg(test)]
mod deltas_test;
