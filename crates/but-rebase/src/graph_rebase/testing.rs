#![deny(missing_docs)]
//! Testing utilities

use petgraph::dot::{Config, Dot};

use crate::graph_rebase::{Editor, Step, StepGraph, rebase::SuccessfulRebase};

/// An extension trait that adds debugging output for graphs
pub trait TestingDot {
    /// Creates a dot graph with labels
    fn steps_dot(&self) -> String;
}

impl TestingDot for Editor {
    fn steps_dot(&self) -> String {
        self.graph.steps_dot()
    }
}

impl TestingDot for SuccessfulRebase {
    fn steps_dot(&self) -> String {
        self.graph.steps_dot()
    }
}

impl TestingDot for StepGraph {
    fn steps_dot(&self) -> String {
        format!(
            "{:?}",
            Dot::with_attr_getters(
                &self,
                &[Config::EdgeNoLabel, Config::NodeNoLabel],
                &|_, v| format!("label=\"order: {}\"", v.weight().order),
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
