mod index;
mod meta;
mod searcher;
mod highlighted;

pub use searcher::{Query, Results, Searcher};

#[cfg(test)]
mod searcher_test;
