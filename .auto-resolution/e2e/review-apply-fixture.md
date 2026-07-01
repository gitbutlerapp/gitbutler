# Review Apply Playwright Fixture

The review-apply Playwright tests exercise the backend-owned `review_apply` API
without talking to real GitHub.

## Moving Parts

- `playwright/src/fakeGithub.ts` starts a small HTTP server that imitates the
  GitHub Enterprise API routes the app needs:
  - `GET /api/v3/user`
  - `GET /api/v3/repos/acme/widgets/pulls`
  - `GET /api/v3/repos/acme/widgets/pulls/42`
- `playwright/scripts/project-with-github-fork-pr.sh` creates the local Git
  repositories used by the tests:
  - `remote-project`, the base repository
  - `fork-project`, a working clone with a fork PR branch
  - `fork-project-bare`, the fork repository fetched by the backend
  - `local-clone`, the GitButler project under test
- `playwright/tests/branches.spec.ts` registers fake GitHub Enterprise
  credentials with `store_github_enterprise_pat`, opens the Branches view, and
  applies PR `#42` through the UI.

## Why GitHub Is Fake But Git Fetch Is Real

The fake GitHub server only serves API JSON. It does not implement Git smart
HTTP. Instead, PR payloads return local filesystem paths as `head.repo.clone_url`
and `head.repo.ssh_url`.

That means the backend still does real work:

- resolves the PR through forge metadata
- creates or reuses a Git remote
- fetches the PR source branch from a real local repository
- applies the fetched remote-tracking branch through workspace apply
- records PR metadata on the applied branch

For fork PRs, the payload points at `fork-project-bare`. For same-repository
PRs, the payload points at `remote-project`.

## Running The Focused Tests

From the repository root:

```bash
cargo build -p but-server
corepack pnpm --filter @gitbutler/e2e exec playwright test \
  --config ./playwright/playwright.config.ts \
  --tsconfig ./playwright/tsconfig.json \
  playwright/tests/branches.spec.ts \
  -g "GitHub review apply"
```

The group covers:

- fork PR apply, creating the fork remote
- fork PR apply, reusing an existing matching remote
- same-repository PR apply from a managed workspace
- same-repository PR apply from single-branch mode
- fork PR apply from single-branch mode

## Adding Cases

Prefer extending `startFakeGitHubServer()` options rather than adding another
fake server. If a new case needs different Git contents, add that branch or file
to `project-with-github-fork-pr.sh` and keep the PR payload pointing at a local
repository path so backend fetch behavior remains real and deterministic.
