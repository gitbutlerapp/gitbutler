# @gitbutler/but-sdk

GitButler Node.js SDK - Access GitButler's Git workflow automation APIs from JavaScript/TypeScript applications.

## Installation

```bash
npm install @gitbutler/but-sdk
# or
pnpm add @gitbutler/but-sdk
# or
yarn add @gitbutler/but-sdk
```

## Usage

```typescript
import { GitButlerContext, stacks, createVirtualBranch } from '@gitbutler/but-sdk';

// Create a context for your project
const ctx = new GitButlerContext('your-project-uuid');

// Get all stacks (virtual branches) in the workspace
const allStacks = stacks(ctx);
console.log(allStacks);

// Create a new virtual branch
const newBranch = createVirtualBranch(ctx, {
  name: 'my-feature'
});
```

## API

### GitButlerContext

The main entry point for all SDK operations. Create a context with your project's ID (UUID).

```typescript
const ctx = new GitButlerContext(projectId: string);
```

### Modern APIs

#### Branch Operations
- `apply(ctx, params)` - Apply a branch to the workspace
- `applyOnly(ctx, params)` - Apply without side effects
- `branchDiff(ctx, params)` - Get the diff for a branch

#### Commit Operations
- `commitReword(ctx, params)` - Reword a commit message
- `commitInsertBlank(ctx, params)` - Insert a blank commit
- `commitMoveChangesBetween(ctx, params)` - Move changes between commits
- `commitUncommitChanges(ctx, params)` - Uncommit changes

#### Diff Operations
- `commitDetails(ctx, params)` - Get commit details with diff

### Legacy APIs

#### Workspace
- `headInfo(ctx)` - Get workspace head information
- `stacks(ctx)` - Get all stacks in the workspace
- `stackDetails(ctx, params)` - Get details for a specific stack
- `branchDetails(ctx, params)` - Get details for a specific branch
- `createCommitFromWorktreeChanges(ctx, params)` - Create a commit
- `discardChanges(ctx, params)` - Discard worktree changes

#### Virtual Branches
- `normalizeBranchName(name)` - Normalize a branch name
- `createVirtualBranch(ctx, params)` - Create a new virtual branch
- `deleteLocalBranch(ctx, params)` - Delete a local branch
- `getBaseBranchData(ctx)` - Get base branch data
- `setBaseBranch(ctx, params)` - Set the base branch
- `unapplyStack(ctx, params)` - Unapply a stack
- `listBranches(ctx, params)` - List all branches
- `squashCommits(ctx, params)` - Squash commits
- `fetchFromRemotes(ctx, params)` - Fetch from remotes
- `moveCommit(ctx, params)` - Move a commit
- `updateCommitMessage(ctx, params)` - Update commit message

## Building from Source

```bash
# Install dependencies
pnpm install

# Build the native module
pnpm build

# Run tests
pnpm test
```

## Platform Support

The SDK provides prebuilt binaries for:
- macOS (arm64, x64)
- Linux (x64 GNU)
- Windows (x64 MSVC)

## License

FSL-1.1-MIT - See [LICENSE.md](../../LICENSE.md) for details.
