#![deny(missing_docs)]
//! Testing utilities

use petgraph::dot::{Config, Dot};

use crate::graph_rebase::{Editor, Step};

impl Editor {
    /// Creates a dot graph with labels
    pub fn steps_dot(&self) -> String {
        format!(
            "{:?}",
            Dot::with_attr_getters(
                &self.graph,
                &[Config::EdgeNoLabel, Config::NodeNoLabel],
                &|e, v| format!("label=\"order: {}\"", v.weight().order),
                &|_, (_, step)| {
                    match step {
                        Step::Pick { id } => format!("label=\"pick: {}\"", id),
                        Step::Reference { refname } => {
                            format!("label=\"reference: {}\"", refname.as_bstr())
                        }
                        Step::None => "label=\"none\"".into(),
                    }
                },
            )
        )
    }
}
