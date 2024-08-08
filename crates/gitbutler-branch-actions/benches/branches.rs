use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use gitbutler_branch_actions::list_branches;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

pub fn fixture_project(name: &str, script: &str) -> Project {
    gitbutler_testsupport::read_only::fixture_project(script, name).unwrap()
}

pub fn benchmark_list_branches(c: &mut Criterion) {
    const NUM_BRANCHES: u64 = "invalid";
    for (bench_name, num_references, (repo_name, script_name)) in [
        (
            "list-branches[many local branches]",
            NUM_BRANCHES,
            ("many-local", "branch-benches.sh"),
        ),
        (
            "list-branches[tiny repo]",
            3,
            ("one-vbranch-on-integration-two-remotes", "for-listing.sh"),
        ),
        (
            "list-branches[many local branches [packed]]",
            NUM_BRANCHES,
            ("many-local-packed", "branch-benches.sh"),
        ),
        (
            "list-branches[many local branches [tracked]]",
            NUM_BRANCHES * 2,
            ("many-local-tracked", "branch-benches.sh"),
        ),
        (
            "list-branches[many local branches [tracked & packed]]",
            NUM_BRANCHES * 2,
            ("many-local-tracked-packed", "branch-benches.sh"),
        ),
    ] {
        let mut group = c.benchmark_group(bench_name);
        let project = fixture_project(repo_name, script_name);
        let ctx = CommandContext::open(&project).unwrap();
        group.throughput(Throughput::Elements(num_references));
        group
            .bench_function("no filter", |b| {
                b.iter(|| list_branches(black_box(&ctx), None, None))
            })
            .bench_function("name-filter rejecting all", |b| {
                b.iter(|| list_branches(black_box(&ctx), None, Some(vec!["not available".into()])))
            });
    }
}

criterion_group!(benches, benchmark_list_branches);
criterion_main!(benches);
