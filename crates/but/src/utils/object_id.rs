use but_core::ChangeId;
use gix::prelude::ObjectIdExt as _;

/// Shorten a commit object id using repository disambiguation (`core.abbrev`), and return
/// the result as a hex-string.
pub fn shorten_object_id(repo: &gix::Repository, oid: impl Into<gix::ObjectId>) -> String {
    oid.into().attach(repo).shorten_or_id().to_string()
}

/// Try to shorten a hex object id string using repository disambiguation.
///
/// If `hex` cannot be parsed as an object id, return it unchanged.
pub fn shorten_hex_object_id(repo: &gix::Repository, hex: &str) -> String {
    gix::ObjectId::from_hex(hex.as_bytes())
        .map(|oid| shorten_object_id(repo, oid))
        .unwrap_or_else(|_| hex.to_owned())
}

/// Read commit's change ID or synthesize one based on the commit ID if none is available.
pub fn get_change_id_for_commit(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> anyhow::Result<ChangeId> {
    let commit = repo.find_commit(commit_id)?;
    let commit = commit.decode()?;
    Ok(
        but_core::commit::Headers::try_from_commit_headers(|| commit.extra_headers())
            .and_then(|headers| headers.change_id)
            .unwrap_or_else(|| {
                but_core::commit::Headers::synthetic_change_id_from_commit_id(commit_id)
            }),
    )
}
