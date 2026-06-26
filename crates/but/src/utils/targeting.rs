//! Utility functions for targeting operations.

use std::fmt::Display;

use but_rebase::graph_rebase::mutate::InsertSide;
use but_workspace::branch::create_reference::Position;

#[derive(Clone, Copy)]
pub(crate) enum Side {
    Above,
    Below,
}

impl Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pretty = match self {
            Self::Above => "above",
            Self::Below => "below",
        };
        write!(f, "{pretty}")
    }
}

impl From<Side> for InsertSide {
    fn from(value: Side) -> Self {
        match value {
            Side::Above => Self::Above,
            Side::Below => Self::Below,
        }
    }
}

impl From<InsertSide> for Side {
    fn from(value: InsertSide) -> Self {
        match value {
            InsertSide::Above => Self::Above,
            InsertSide::Below => Self::Below,
        }
    }
}

impl From<Side> for Position {
    fn from(value: Side) -> Self {
        match value {
            Side::Above => Self::Above,
            Side::Below => Self::Below,
        }
    }
}
