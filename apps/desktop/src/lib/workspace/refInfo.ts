import type { WorkspaceRefInfo, Workspace } from '@gitbutler/core/api';

export type RefInfo = WorkspaceRefInfo.RefInfo;

export function stackDetailsFromRefInfo(refInfo: RefInfo): Workspace.StackDetails[] {
	const stacks: Workspace.StackDetails[] = [];
	outer: for (const stack of refInfo.stacks) {
		const branchDetails: Workspace.BranchDetails[] = [];
		for (const segment of stack.segments) {
			const name = segment.refName?.displayName;
			// Skip stacks with no branch names
			if (!name) continue outer;
			const remoteRefRemote = segment.remoteTrackingRefName?.remoteName;
			const remoteRefBranch = segment.remoteTrackingRefName?.displayName;
			const remoteTrackingRefName =
				!!remoteRefRemote && !!remoteRefBranch
					? `refs/remotes/${remoteRefRemote}/${remoteRefBranch}`
					: null;
			const base = segment.base;
			// Skip stacks with no base
			if (!base) continue outer;
			const tip = segment.commits.at(0)?.id ?? segment.base;
			/// Skip stacks with no tip
			if (!tip) continue outer;
			branchDetails.push({
				name,
				linkedWorktreeId: null,
				remoteTrackingBranch,
				description: segment.metadata?.description ?? null,
				prNumber: segment.metadata?.review.pullRequest ?? null,
				reviewId: segment.metadata?.review.reviewId ?? null,
				tip,
				baseCommit: base,
				pushStatus: segment.pushStatus
			});
		}
		if (branchDetails.length === 0) continue;
	}

	return stacks;
}
