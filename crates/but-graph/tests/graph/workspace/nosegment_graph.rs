use but_graph::Graph;
use but_testsupport::visualize_commit_graph_all;

use super::target_meta;
use crate::init::utils::{StackState, add_stack, read_only_in_memory_scenario, standard_options};

#[test]
fn walk_stops_below_the_target_commit() -> anyhow::Result<()> {
    let (repo, meta, mut db) = read_only_in_memory_scenario("history-below-target")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 6bfd487 (HEAD -> A) A
* 203882d (origin/main, main) M
* 75fb04b old-2
* cb41913 old-1

"#]]
    );
    let ws = Graph::from_head(
        &repo,
        &*meta,
        target_meta(&repo),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    let graph = but_graph::nosegment::workspace_segment_graph(
        &ws,
        &repo,
        &*meta,
        vec![repo.head_id()?.detach()],
    )?;
    let mut subjects = Vec::new();
    for sidx in graph.segments() {
        for commit in &graph[sidx].commits {
            subjects.push(
                repo.find_commit(commit.id)?
                    .message()?
                    .summary()
                    .to_string(),
            );
        }
    }
    subjects.sort();
    assert_eq!(
        subjects,
        ["A", "M", "old-2"],
        "the walk ends right below the target commit, which no tip needs it to go past"
    );
    Ok(())
}

#[test]
fn commits_limit_bounds_only_a_walk_without_target_commit() -> anyhow::Result<()> {
    let (repo, meta, _db) = read_only_in_memory_scenario("history-below-target")?;
    let head_ref = repo.head_name()?;
    let head_id = repo.head_id()?.detach();
    // The first commit, so the walk down to it takes longer than the limit.
    let target_commit_id = repo.rev_parse_single("A~3")?.detach();
    // No target branch, so the target commit isn't walked as a head that the limit can't cut.
    let subjects = |target_commit_id| -> anyhow::Result<Vec<String>> {
        let graph = but_graph::nosegment::segment_graph(
            &repo,
            &*meta,
            but_graph::init::Overlay::default(),
            Default::default(),
            head_ref.as_ref().map(|name| name.as_ref()),
            Some(head_id),
            vec![head_id],
            Vec::new(),
            target_commit_id,
            false,
            Some(1),
        )?;
        let mut subjects = Vec::new();
        for sidx in graph.segments() {
            for commit in &graph[sidx].commits {
                subjects.push(
                    repo.find_commit(commit.id)?
                        .message()?
                        .summary()
                        .to_string(),
                );
            }
        }
        subjects.sort();
        Ok(subjects)
    };

    assert_eq!(
        subjects(None)?,
        ["A", "M"],
        "without a target commit, the walk stops after the limit, at a commit that keeps its parents"
    );
    assert_eq!(
        subjects(Some(target_commit_id))?,
        ["A", "M", "old-1", "old-2"],
        "the target commit bounds the walk instead of the limit"
    );
    Ok(())
}

#[test]
fn segment_construction_reuses_walked_commits_and_their_boundary_parents() -> anyhow::Result<()> {
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Reads(Arc<Mutex<Vec<gix::ObjectId>>>);
    impl gix::odb::pack::cache::Object for Reads {
        fn put(&mut self, _: gix::ObjectId, _: gix::objs::Kind, _: &[u8]) {}

        fn get(&mut self, id: &gix::ObjectId, _: &mut Vec<u8>) -> Option<gix::objs::Kind> {
            self.0.lock().unwrap().push(*id);
            None
        }
    }
    let (mut repo, meta, _db) = read_only_in_memory_scenario("history-below-target")?;
    let head = repo.head_id()?.detach();
    let walked = repo.rev_parse_single("A~1")?.detach();
    let boundary = repo.rev_parse_single("A~2")?.detach();
    let beyond = repo.rev_parse_single("A~3")?.detach();
    let reads = Reads::default();
    repo.objects.set_object_cache({
        let reads = reads.clone();
        move || Box::new(reads.clone())
    });
    let graph = but_graph::nosegment::segment_graph(
        &repo,
        &*meta,
        Default::default(),
        Default::default(),
        None,
        Some(head),
        vec![head],
        Vec::new(),
        None,
        false,
        Some(2),
    )?;
    let commits: Vec<_> = graph.segments().flat_map(|s| &graph[s].commits).collect();
    assert_eq!(commits.len(), 3);
    let reads = reads.0.lock().unwrap();
    for (id, parent) in [(walked, boundary), (boundary, beyond)] {
        assert_eq!(
            commits
                .iter()
                .find(|c| c.id == id)
                .unwrap()
                .parent_ids
                .as_slice(),
            [parent]
        );
        assert_eq!(
            reads.iter().filter(|read| **read == id).count(),
            1,
            "reads of {id}"
        );
    }
    Ok(())
}

#[test]
fn shallow_clone_keeps_the_recorded_parent_without_walking_it() -> anyhow::Result<()> {
    let (repo, meta, _db) = crate::init::utils::named_read_only_in_memory_scenario(
        "special-conditions",
        "shallow-clone-ahead-of-upstream",
    )?;
    let head = repo.head_id()?.detach();
    let boundary = repo.shallow_commits()?.unwrap().head;
    let parent = repo
        .find_commit(boundary)?
        .parent_ids()
        .next()
        .unwrap()
        .detach();
    let graph = but_graph::nosegment::segment_graph(
        &repo,
        &*meta,
        Default::default(),
        Default::default(),
        None,
        Some(head),
        vec![head],
        Vec::new(),
        None,
        false,
        None,
    )?;
    let boundary = graph
        .segment_by_commit_id(boundary)?
        .commits
        .iter()
        .find(|c| c.id == boundary)
        .unwrap();
    assert!(
        boundary
            .flags
            .contains(but_graph::CommitFlags::ShallowBoundary)
    );
    assert_eq!(boundary.parent_ids.as_slice(), [parent]);
    assert!(graph.segment_id_by_commit_id(parent).is_err());
    Ok(())
}

#[test]
fn octopus_merge_keeps_all_parents_in_git_order() -> anyhow::Result<()> {
    let (repo, meta, _) = read_only_in_memory_scenario("history-below-target")?;
    let repo = repo.with_object_memory();
    let mut octopus = repo.head_commit()?.decode()?.to_owned()?;
    octopus.parents = ["A~2", "A", "A~1"]
        .into_iter()
        .map(|rev| repo.rev_parse_single(rev).map(gix::Id::detach))
        .collect::<gix::Result<_>>()?;
    let head = repo.write_object(&octopus)?.detach();
    let graph = but_graph::nosegment::segment_graph(
        &repo,
        &*meta,
        Default::default(),
        Default::default(),
        None,
        Some(head),
        vec![head],
        Vec::new(),
        None,
        false,
        None,
    )?;
    let commit = graph
        .segment_by_commit_id(head)?
        .commits
        .iter()
        .find(|commit| commit.id == head)
        .unwrap();
    assert_eq!(commit.parent_ids.as_slice(), octopus.parents.as_slice());
    Ok(())
}

#[test]
fn placing_branches_does_not_read_history_beyond_the_walk() -> anyhow::Result<()> {
    struct RejectRead(gix::ObjectId);
    impl gix::odb::pack::cache::Object for RejectRead {
        fn put(&mut self, _: gix::ObjectId, _: gix::objs::Kind, _: &[u8]) {}

        fn get(&mut self, id: &gix::ObjectId, _: &mut Vec<u8>) -> Option<gix::objs::Kind> {
            assert_ne!(
                *id, self.0,
                "placing refs must not walk back to the common ancestor outside the graph"
            );
            None
        }
    }

    let (mut repo, mut meta, _db) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    // A and main diverge at init. A walk limited to its heads includes neither path to init.
    let common_ancestor = repo.rev_parse_single("main~2")?.detach();
    let workspace = repo.head_id()?.detach();
    repo.objects
        .set_object_cache(move || Box::new(RejectRead(common_ancestor)));
    let graph = but_graph::nosegment::segment_graph(
        &repo,
        &*meta,
        Default::default(),
        Default::default(),
        repo.head_name()?.as_ref().map(|name| name.as_ref()),
        Some(workspace),
        vec![workspace],
        Vec::new(),
        None,
        false,
        Some(0),
    )?;
    assert!(
        graph.segment_id_by_commit_id(common_ancestor).is_err(),
        "the common ancestor is beyond the walk's boundary"
    );
    Ok(())
}

#[test]
fn a_stack_integrated_only_into_the_targets_upstream_keeps_its_lane() -> anyhow::Result<()> {
    let (repo, mut meta, _db) =
        read_only_in_memory_scenario("ws/single-stack-renamed-target-upstream")?;
    add_stack(&mut meta, 1, "B", StackState::InWorkspace);
    add_stack(&mut meta, 2, "A", StackState::InWorkspace);
    let ws = but_graph::nosegment::workspace(
        &repo,
        &*meta,
        Default::default(),
        target_meta(&repo),
        repo.head_name()?.as_ref().map(|name| name.as_ref()),
        Some(repo.head_id()?.detach()),
        [],
        Vec::new(),
        false,
        None,
    )?;
    assert_eq!(
        ws.stacks
            .iter()
            .filter_map(|stack| stack.ref_name())
            .map(|name| name.shorten().to_string())
            .collect::<Vec<_>>(),
        ["B", "A"],
        "origin/trunk contains A, so A is a separate lane even though B is stacked on it"
    );
    let mut inferred = target_meta(&repo);
    inferred.target_commit_id = None;
    assert_eq!(
        inferred.resolve_target_commit(&repo)?,
        repo.rev_parse_single("corrected^{commit}")?.detach(),
        "an inferred cutoff follows the same upstream as stack placement"
    );
    Ok(())
}
