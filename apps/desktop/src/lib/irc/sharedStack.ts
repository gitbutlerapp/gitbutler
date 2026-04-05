import type { TreeChange } from "$lib/hunks/change";
import type { UnifiedDiff } from "$lib/hunks/diff";
import type { DiffService } from "$lib/hunks/diffService.svelte";
import type { DiffHunk } from "$lib/hunks/hunk";
import type { StackService } from "$lib/stacks/stackService.svelte";

// ============================================================================
// Patch Construction (for git apply)
// ============================================================================

/** Build a git-apply-compatible patch string for a single hunk. */
export function buildHunkPatch(change: TreeChange, hunk: DiffHunk): string {
	return `--- a/${change.path}\n+++ b/${change.path}\n${hunk.diff}`;
}

/** Build a git-apply-compatible patch string for all files in a shared commit. */
export function buildCommitPatch(commit: SharedCommitPayload): string {
	return commit.commit.files
		.flatMap((f) => f.hunks.map((h) => buildHunkPatch(f.change, h)))
		.join("\n");
}

// ============================================================================
// Payload Types
// ============================================================================

export type SharedCommit = {
	id: string;
	message: string;
	author: { name: string; email: string };
	createdAt: number;
	files: SharedFile[];
};

export type SharedFile = {
	path: string;
	change: TreeChange;
	hunks: DiffHunk[];
};

export type SharedCommitPayload = {
	commitId: string;
	projectName: string;
	commit: SharedCommit;
};

// ============================================================================
// Builder
// ============================================================================

/**
 * Fetch a single commit's data (metadata + diffs) and assemble a
 * `SharedCommitPayload` for sending over IRC.
 */
export async function buildSharedCommitPayload(
	stackId: string,
	commitId: string,
	projectId: string,
	projectName: string,
	stackService: StackService,
	diffService: DiffService,
): Promise<SharedCommitPayload> {
	// Find the commit object in the stack branches (for author/createdAt)
	const branches = await stackService.fetchBranches(projectId, stackId);
	let commitMeta:
		| { message: string; author: { name: string; email: string }; createdAt: number }
		| undefined;
	for (const branch of branches ?? []) {
		const found = branch.commits.find((c) => c.id === commitId);
		if (found) {
			commitMeta = {
				message: found.message,
				author: { name: found.author.name, email: found.author.email },
				createdAt: Number(found.createdAt),
			};
			break;
		}
	}

	// Fetch file changes and diffs
	const commitResult = await stackService.fetchCommitChanges(projectId, commitId);
	const changes = commitResult?.changes ?? [];
	const sharedFiles: SharedFile[] = [];

	for (const change of changes) {
		const hunks: DiffHunk[] = [];
		const diff: UnifiedDiff | null = await diffService.fetchDiff(projectId, change);
		if (diff?.type === "Patch") {
			hunks.push(...diff.subject.hunks);
		}
		sharedFiles.push({ path: change.path, change, hunks });
	}

	return {
		commitId,
		projectName,
		commit: {
			id: commitId,
			message: commitMeta?.message ?? "",
			author: commitMeta?.author ?? { name: "Unknown", email: "" },
			createdAt: commitMeta?.createdAt ?? Date.now(),
			files: sharedFiles,
		},
	};
}
