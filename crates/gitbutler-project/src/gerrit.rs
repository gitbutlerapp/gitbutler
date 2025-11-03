/// Check if the default remote of the repository is using Gerrit.
pub fn is_used_by_default_remote(repo: &gix::Repository) -> anyhow::Result<bool> {
    use gix::{bstr::ByteSlice, remote::Direction};

    // Magic refspec that we use to determine if the remote is a Gerrit remote
    let gerrit_notes_ref = "refs/notes/review";

    let remote_name = repo
        .remote_default_name(Direction::Push)
        .ok_or_else(|| anyhow::anyhow!("No push remotes found"))?;

    let mut remote = repo.find_remote(remote_name.as_bstr())?;
    // Need to set fetch specs as ref-map requests always use fetch specs (it's not used when pushing)
    remote.replace_refspecs(vec![gerrit_notes_ref], Direction::Fetch)?;

    let (map, _) = remote
        .with_fetch_tags(gix::remote::fetch::Tags::None)
        .connect(Direction::Push)?
        .ref_map(gix::progress::Discard, Default::default())?;

    Ok(!map.remote_refs.is_empty())
}
