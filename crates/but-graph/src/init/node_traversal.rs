use std::collections::{BTreeSet, HashMap};

use anyhow::{Result, ensure};

use crate::{BoundaryKind, Node, NodeIndex, NodeKind};

use super::{OverlayRepo, remotes, walk::try_refname_to_id};

/// Build the smallest commit topology that connects `tips`.
///
/// Configured upstreams discovered at local refs in the selected topology add
/// ordinary object IDs. Selection restarts until that set is stable.
pub(super) fn traverse(
    repo: &OverlayRepo<'_>,
    tips: impl IntoIterator<Item = gix::ObjectId>,
) -> Result<Vec<Node>> {
    let mut tips = tips.into_iter().collect::<BTreeSet<_>>();
    ensure!(!tips.is_empty(), "commit traversal needs at least one tip");

    let shallow = repo
        .shallow_commits()?
        .map(|commits| commits.iter().copied().collect())
        .unwrap_or_default();
    let upstreams_by_commit = upstreams_by_commit(repo)?;

    let selected = loop {
        let selected = select_commits(repo, &tips, &shallow)?;
        let before = tips.len();
        for id in &selected.order {
            tips.extend(upstreams_by_commit.get(id).into_iter().flatten().copied());
        }
        if tips.len() == before {
            break selected;
        }
    };

    Ok(materialize(selected, &shallow))
}

struct Selection {
    order: Vec<gix::ObjectId>,
    parents: HashMap<gix::ObjectId, Vec<gix::ObjectId>>,
}

fn select_commits(
    repo: &OverlayRepo<'_>,
    tips: &BTreeSet<gix::ObjectId>,
    shallow: &BTreeSet<gix::ObjectId>,
) -> Result<Selection> {
    let base = if tips.len() > 1 {
        match repo
            .for_find_only()
            .merge_base_octopus(tips.iter().copied())
        {
            Ok(base) => Some(base.detach()),
            Err(gix::repository::merge_base_octopus::Error::MergeBaseOctopus(
                gix::repository::merge_base_octopus_with_graph::Error::NoMergeBase,
            )) => None,
            Err(err) => return Err(err.into()),
        }
    } else {
        None
    };

    let hidden = match base {
        Some(base) if !shallow.contains(&base) => repo
            .find_commit(base)?
            .parent_ids()
            .map(|id| id.detach())
            .collect(),
        Some(_) | None => Vec::new(),
    };
    let mut walk = repo.for_find_only().rev_walk(tips.iter().copied());
    if !hidden.is_empty() {
        walk = walk.with_hidden(hidden);
    }

    let mut order = Vec::new();
    let mut parents = HashMap::new();
    for info in walk.all()? {
        let info = info?;
        order.push(info.id);
        parents.insert(info.id, info.parent_ids.iter().copied().collect());
    }
    Ok(Selection { order, parents })
}

fn upstreams_by_commit(
    repo: &OverlayRepo<'_>,
) -> Result<HashMap<gix::ObjectId, BTreeSet<gix::ObjectId>>> {
    let local_refs = repo.collect_ref_mapping_by_prefix(["refs/heads/"].into_iter(), &[])?;
    let effective_upstreams = remotes::effective_remote_tracking_branches(repo)?;
    let mut out = HashMap::<_, BTreeSet<_>>::new();
    for (commit_id, names) in local_refs {
        for name in names {
            let Some(upstream) = effective_upstreams.get(&name) else {
                continue;
            };
            if let Some(upstream_id) = try_refname_to_id(repo, upstream.as_ref())? {
                out.entry(commit_id).or_default().insert(upstream_id);
            }
        }
    }
    Ok(out)
}

fn materialize(selected: Selection, shallow: &BTreeSet<gix::ObjectId>) -> Vec<Node> {
    let Selection { order, parents } = selected;
    let mut nodes = Vec::with_capacity(order.len());
    let mut index_by_id = HashMap::<gix::ObjectId, NodeIndex>::new();
    for id in &order {
        index_by_id.insert(*id, nodes.len());
        nodes.push(Node {
            kind: NodeKind::Commit { id: *id },
            parents: Vec::new(),
        });
    }

    let mut boundary_by_key = HashMap::<(gix::ObjectId, BoundaryKind), NodeIndex>::new();
    for id in order {
        let child = index_by_id[&id];
        let reason = if shallow.contains(&id) {
            BoundaryKind::Shallow
        } else {
            BoundaryKind::Convergence
        };
        let mut ordered_parents = Vec::new();
        for parent_id in parents.get(&id).into_iter().flatten() {
            let parent = match index_by_id.get(parent_id) {
                Some(index) => *index,
                None => *boundary_by_key
                    .entry((*parent_id, reason))
                    .or_insert_with(|| {
                        let index = nodes.len();
                        nodes.push(Node {
                            kind: NodeKind::Boundary {
                                id: *parent_id,
                                reason,
                            },
                            parents: Vec::new(),
                        });
                        index
                    }),
            };
            ordered_parents.push(parent);
        }
        nodes[child].parents = ordered_parents;
    }
    nodes
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use but_testsupport::InMemoryRefMetadata;
    use gix::refs::Target;

    use super::*;
    use crate::init::Overlay;

    fn scenario(script: &str, name: &str) -> Result<gix::Repository> {
        let root = but_testsupport::gix_testtools::scripted_fixture_read_only(script)
            .map_err(anyhow::Error::from_boxed)?;
        Ok(gix::open_opts(root.join(name), gix::open::Options::isolated())?.with_object_memory())
    }

    fn id(repo: &gix::Repository, spec: &str) -> Result<gix::ObjectId> {
        Ok(repo.rev_parse_single(spec)?.object()?.peel_to_commit()?.id)
    }

    fn commit_ids(nodes: &[Node]) -> BTreeSet<gix::ObjectId> {
        nodes
            .iter()
            .filter_map(|node| match node.kind {
                NodeKind::Commit { id } => Some(id),
                NodeKind::Reference(_) | NodeKind::Boundary { .. } | NodeKind::None => None,
            })
            .collect()
    }

    fn traverse_with(
        repo: &gix::Repository,
        overlay: Overlay,
        tips: impl IntoIterator<Item = gix::ObjectId>,
    ) -> Result<Vec<Node>> {
        let meta = InMemoryRefMetadata::default();
        let (repo, _meta, _entrypoint) = overlay.into_parts(repo, &meta);
        traverse(&repo, tips)
    }

    #[test]
    fn a_single_tip_has_real_roots_and_deduplicates_tips() -> Result<()> {
        let repo = scenario("scenarios.sh", "triple-merge")?;
        let tip = id(&repo, "A")?;
        let nodes = traverse_with(&repo, Overlay::default(), [tip, tip])?;
        let commits = commit_ids(&nodes);

        assert_eq!(
            commits.len(),
            nodes.len(),
            "a full closure has no boundaries"
        );
        assert_eq!(commits.len(), 8, "duplicate tips do not duplicate commits");
        assert!(nodes.iter().any(|node| {
            matches!(node.kind, NodeKind::Commit { .. }) && node.parents.is_empty()
        }));
        Ok(())
    }

    #[test]
    fn overlapping_tips_materialize_each_selected_commit_once() -> Result<()> {
        let repo = scenario("scenarios.sh", "triple-merge")?;
        let tip = id(&repo, "A")?;
        let ancestor = id(&repo, "A~1")?;
        let nodes = traverse_with(&repo, Overlay::default(), [tip, ancestor])?;
        let commits = commit_ids(&nodes);

        assert_eq!(commits, BTreeSet::from([tip, ancestor]));
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(
                    node.kind,
                    NodeKind::Boundary {
                        reason: BoundaryKind::Convergence,
                        ..
                    }
                ))
                .count(),
            1
        );
        Ok(())
    }

    #[test]
    fn disconnected_tips_keep_both_full_closures() -> Result<()> {
        let repo = scenario("scenarios.sh", "multi-root")?;
        let tips = [id(&repo, "B")?, id(&repo, "D")?];
        let nodes = traverse_with(&repo, Overlay::default(), tips)?;

        assert_eq!(commit_ids(&nodes), BTreeSet::from(tips));
        assert!(
            nodes
                .iter()
                .all(|node| !matches!(node.kind, NodeKind::Boundary { .. }))
        );
        Ok(())
    }

    #[test]
    fn a_missing_tip_is_an_error() -> Result<()> {
        let repo = scenario("scenarios.sh", "multi-root")?;
        let missing = gix::ObjectId::from_hex(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")?;
        assert!(traverse_with(&repo, Overlay::default(), [missing]).is_err());
        Ok(())
    }

    #[test]
    fn octopus_convergence_preserves_every_parent_slot_in_git_order() -> Result<()> {
        let repo = scenario("scenarios.sh", "three-tip-octopus-base")?;
        let tips = [id(&repo, "A")?, id(&repo, "B")?, id(&repo, "C")?];
        let base = id(&repo, "base")?;
        let expected = repo
            .find_commit(base)?
            .parent_ids()
            .map(|id| id.detach())
            .collect::<Vec<_>>();
        let nodes = traverse_with(&repo, Overlay::default(), tips)?;
        let base_node = nodes
            .iter()
            .find(|node| matches!(node.kind, NodeKind::Commit { id } if id == base))
            .expect("merge base is selected");
        let actual = base_node
            .parents
            .iter()
            .map(|index| match nodes[*index].kind {
                NodeKind::Boundary {
                    id,
                    reason: BoundaryKind::Convergence,
                } => id,
                ref other => panic!("expected convergence boundary, got {other:?}"),
            })
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 3);
        assert_eq!(actual.iter().copied().collect::<BTreeSet<_>>().len(), 3);
        Ok(())
    }

    #[test]
    fn configured_upstreams_reach_a_fixed_point() -> Result<()> {
        let repo = scenario("scenarios.sh", "configured-upstream-fixed-point")?;
        let tip = id(&repo, "B")?;
        let nodes = traverse_with(&repo, Overlay::default(), [tip])?;
        let commits = commit_ids(&nodes);

        for spec in ["A", "B", "origin/A", "origin/B"] {
            assert!(commits.contains(&id(&repo, spec)?), "missing {spec}");
        }
        Ok(())
    }

    #[test]
    fn unique_same_name_fallback_is_traversed() -> Result<()> {
        let repo = scenario("scenarios.sh", "effective-upstream-rules")?;
        let tip = id(&repo, "unique")?;
        let remote = id(&repo, "origin/unique")?;
        let commits = commit_ids(&traverse_with(&repo, Overlay::default(), [tip])?);

        assert!(commits.contains(&tip));
        assert!(
            commits.contains(&remote),
            "fallback upstream tip is selected"
        );
        Ok(())
    }

    #[test]
    fn fallback_upstream_honors_moved_and_dropped_overlay_refs() -> Result<()> {
        let repo = scenario("scenarios.sh", "effective-upstream-rules")?;
        let tip = id(&repo, "unique")?;
        let old_remote = id(&repo, "origin/unique")?;
        let moved_remote = id(&repo, "origin/configured")?;
        let moved = Overlay::default().with_references([gix::refs::Reference {
            name: "refs/remotes/origin/unique".try_into()?,
            target: Target::Object(moved_remote),
            peeled: Some(moved_remote),
        }]);
        let moved_commits = commit_ids(&traverse_with(&repo, moved, [tip])?);
        assert!(moved_commits.contains(&moved_remote));
        assert!(!moved_commits.contains(&old_remote));

        let dropped =
            Overlay::default().with_dropped_references(["refs/remotes/origin/unique".try_into()?]);
        let dropped_commits = commit_ids(&traverse_with(&repo, dropped, [tip])?);
        assert!(!dropped_commits.contains(&old_remote));
        assert!(!dropped_commits.contains(&moved_remote));
        Ok(())
    }

    #[test]
    fn upstream_lookup_honors_moved_and_dropped_overlay_refs() -> Result<()> {
        let repo = scenario("scenarios.sh", "configured-upstream-fixed-point")?;
        let tip = id(&repo, "B")?;
        let old_remote = id(&repo, "origin/B")?;
        let moved_remote = id(&repo, "origin/A")?;
        let moved = Overlay::default().with_references([gix::refs::Reference {
            name: "refs/remotes/origin/B".try_into()?,
            target: Target::Object(moved_remote),
            peeled: Some(moved_remote),
        }]);
        let moved_commits = commit_ids(&traverse_with(&repo, moved, [tip])?);
        assert!(moved_commits.contains(&moved_remote));
        assert!(!moved_commits.contains(&old_remote));

        let dropped =
            Overlay::default().with_dropped_references(["refs/remotes/origin/B".try_into()?]);
        let dropped_commits = commit_ids(&traverse_with(&repo, dropped, [tip])?);
        assert!(!dropped_commits.contains(&old_remote));
        assert!(!dropped_commits.contains(&moved_remote));
        Ok(())
    }

    #[test]
    fn shallow_boundary_wins_over_convergence() -> Result<()> {
        let repo = scenario("special-conditions.sh", "shallow-clone-depth-2")?;
        let tip = id(&repo, "HEAD")?;
        let shallow = repo.shallow_commits()?.expect("shallow clone").head;
        let nodes = traverse_with(&repo, Overlay::default(), [tip, shallow])?;
        let shallow_node = nodes
            .iter()
            .find(|node| matches!(node.kind, NodeKind::Commit { id } if id == shallow))
            .expect("shallow commit is selected");

        assert!(!shallow_node.parents.is_empty());
        assert!(shallow_node.parents.iter().all(|index| matches!(
            nodes[*index].kind,
            NodeKind::Boundary {
                reason: BoundaryKind::Shallow,
                ..
            }
        )));
        Ok(())
    }
}
