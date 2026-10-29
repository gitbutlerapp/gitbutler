use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{Context as _, Result, bail};
use git_meta_lib::{LocalPublish, MetaValue, Session, Target};

use super::{
    INDEX_NAMESPACE, IndexHit, PublicationStatus, SESSION_SET_KEY, local_storage_key,
    session_storage_prefix, strip_local_storage_prefix,
};

struct SetMemberShare {
    local_key: String,
    published_key: String,
    members: Vec<String>,
}

pub(crate) fn share_sessions(repo_path: &Path, session_keys: &[String]) -> Result<()> {
    let session_keys = session_keys
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if session_keys.is_empty() {
        return Ok(());
    }

    let gitmeta = Session::open(repo_path).context("failed to open GitMeta session")?;
    let target = Target::project();
    let handle = gitmeta.target(&target);

    let prefix_shares = session_keys
        .iter()
        .map(|session_key| {
            (
                session_storage_prefix(PublicationStatus::LocalOnly, session_key),
                session_storage_prefix(PublicationStatus::Published, session_key),
            )
        })
        .collect::<Vec<_>>();
    let mut set_member_shares = vec![SetMemberShare {
        local_key: local_storage_key(SESSION_SET_KEY),
        published_key: SESSION_SET_KEY.to_owned(),
        members: session_keys.clone(),
    }];
    set_member_shares.extend(index_set_member_shares(&handle, &session_keys)?);

    let entries = prefix_shares
        .iter()
        .map(|(local, published)| LocalPublish::key_prefix(local, published))
        .chain(set_member_shares.iter().map(|share| {
            LocalPublish::set_members(&share.local_key, &share.published_key, &share.members)
        }))
        .collect::<Vec<_>>();
    handle
        .publish_local(entries)
        .context("failed to share local agentlog metadata")?;
    Ok(())
}

fn index_set_member_shares(
    handle: &git_meta_lib::SessionTargetHandle<'_>,
    session_keys: &[String],
) -> Result<Vec<SetMemberShare>> {
    let selected_sessions = session_keys
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let index_prefix = local_storage_key(INDEX_NAMESPACE);
    let mut shares = BTreeMap::<String, SetMemberShare>::new();
    for (local_key, value) in handle
        .get_all_values(Some(&index_prefix))
        .with_context(|| format!("failed to read GitMeta keys under '{index_prefix}'"))?
    {
        let MetaValue::Set(members) = value else {
            bail!("existing GitMeta key '{local_key}' is not a set");
        };
        let Some(published_key) = strip_local_storage_prefix(&local_key) else {
            continue;
        };
        let published_key = published_key.to_owned();
        let selected_members = members
            .into_iter()
            .filter(|member| match serde_json::from_str::<IndexHit>(member) {
                Ok(hit) => selected_sessions.contains(hit.session_key.as_str()),
                Err(_) => false,
            })
            .collect::<Vec<_>>();
        if selected_members.is_empty() {
            continue;
        }
        shares.insert(
            local_key.clone(),
            SetMemberShare {
                local_key,
                published_key,
                members: selected_members,
            },
        );
    }
    Ok(shares.into_values().collect())
}
