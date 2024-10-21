mod heads;
mod series;
mod stack_ext;
pub use series::Series;
pub use stack_ext::{
    commit_by_oid_or_change_id, CommitsForId, PatchReferenceUpdate, StackExt, TargetUpdate,
};
