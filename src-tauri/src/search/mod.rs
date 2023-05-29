mod index;
mod meta;
mod searcher;

pub use searcher::{Query, Results, Searcher};

#[cfg(test)]
mod searcher_test;
