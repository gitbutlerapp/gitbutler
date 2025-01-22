use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use gitbutler_branch_actions::{get_branch_listing_details, list_branches};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_settings::AppSettings;

pub fn fixture_project(name: &str, script: &str) -> Project {
    gitbutler_testsupport::read_only::fixture_project(script, name).unwrap()
}

pub fn benchmark_list_branches(c: &mut Criterion) {
    const NUM_BRANCHES: u64 = 300;
    for (bench_name, num_references, (repo_name, script_name)) in [
        (
            "list-branches[many local branches]",
            NUM_BRANCHES,
            ("many-local", "branch-benches.sh"),
        ),
        (
            "list-branches[tiny repo]",
            3,
            ("one-vbranch-in-workspace-two-remotes", "for-listing.sh"),
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
        let ctx = CommandContext::open(&project, AppSettings::default()).unwrap();
        group.throughput(Throughput::Elements(num_references));
        group
            .bench_function("no filter", |b| {
                b.iter(|| list_branches(black_box(&ctx), None, None))
            })
            .bench_function("name-filter rejecting all", |b| {
                b.iter(|| list_branches(black_box(&ctx), None, Some(vec!["not-available".into()])))
            });
    }
}

pub fn benchmark_branch_details(c: &mut Criterion) {
    for (bench_name, (repo_name, branch_name, files_to_diff, script_name)) in [
        (
            "branch-details [many branches no change]",
            ("many-local", "virtual", 0, "branch-benches.sh"),
        ),
        (
            "branch-details [tiny no change]",
            (
                "one-vbranch-in-workspace-two-remotes",
                "main",
                0,
                "for-listing.sh",
            ),
        ),
        (
            "branch-details [big repo no change]",
            (
                "big-repo-clone",
                "no-change",
                0,
                "branch-details-benches.sh",
            ),
        ),
        (
            "branch-details [every-file-changed]",
            (
                "big-repo-clone-one-commit-ahead",
                "change-with-new-content",
                10_000,
                "branch-details-benches.sh",
            ),
        ),
    ] {
        let mut group = c.benchmark_group(bench_name);
        let project = fixture_project(repo_name, script_name);
        if files_to_diff != 0 {
            group.throughput(Throughput::Elements(files_to_diff));
        }
        group.bench_function("list details of known branch", |b| {
            b.iter(|| {
                let ctx = CommandContext::open(&project, AppSettings::default()).unwrap();
                let details =
                    get_branch_listing_details(black_box(&ctx), Some(branch_name)).unwrap();
                assert_eq!(details.len(), 1, "{script_name}:{repo_name}:{branch_name}");
                assert_eq!(
                    details[0].number_of_files, files_to_diff as usize,
                    "currently it creates a new vbranch for changes in local-commits, something we leverage here"
                );
            })
        });
    }

    let mut group = c.benchmark_group("branch-details [revwalk]");
    let project = fixture_project("revwalk-repo", "branch-details-benches.sh");
    group.throughput(Throughput::Elements(100 + 15 + 50));
    group.bench_function("count commits/collect authors", |b| {
        b.iter(|| {
            let ctx = CommandContext::open(&project, AppSettings::default()).unwrap();
            let details = get_branch_listing_details(
                black_box(&ctx),
                ["feature", "main", "non-virtual-feature"],
            )
            .unwrap();
            assert_eq!(details.len(), 3);
        })
    });
}

criterion_group!(benches, benchmark_list_branches, benchmark_branch_details);
criterion_main!(benches);
