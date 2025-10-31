import { showToast } from '$lib/notifications/toasts';
import { TestId } from '@gitbutler/ui';
import type { Author, Commit, UpstreamCommit } from '$lib/branches/v3';
import type { CellType } from '@gitbutler/ui/components/commitLines/types';
import type iconsJson from '@gitbutler/ui/data/icons.json';

export type CreateBranchFromBranchOutcome = {
	stackId: string;
	unappliedStacks: string[];
};

export function handleCreateBranchFromBranchOutcome(outcome: CreateBranchFromBranchOutcome) {
	if (outcome.unappliedStacks.length > 0) {
		showToast({
			testId: TestId.StacksUnappliedToast,
			title: 'Heads up: We had to unapply some stacks to apply this one',
			message: `There were some conflicts detected when applying this branch into your workspace, so we automatically unapplied ${outcome.unappliedStacks.length} ${outcome.unappliedStacks.length === 1 ? 'stack' : 'stacks'}.
You can always re-apply them later from the branches page.`
		});
	}
}

export type StackHeadInfo = {
	/**
	 * The name of the branch
	 */
	readonly name: string;
	/**
	 * The commit hash of the tip of the branch
	 */
	readonly tip: string;
};

/**
 * Return type of Tauri `stacks` command.
 */
export type Stack = {
	/**
	 * The id of the stack.
	 */
	id?: string;
	/**
	 * Information about the branches contained in the stack.
	 */
	heads: StackHeadInfo[];
	/**
	 * The commit hash of the tip of the stack.
	 */
	tip: string;
	/**
	 * Zero-based index for sorting the stacks.
	 */
	order: number;
};

export type GerritPushFlag =
	| { type: 'wip' }
	| { type: 'ready' }
	| { type: 'private' }
	| { type: 'hashtag'; subject: string }
	| { type: 'topic'; subject: string };

/**
 * Return (future) type of Tauri `stacks` command.
 * It's currently used to assure the frontend doesn't accidentally see
 * an optional stack-id yet, and to show what it would have to be.
 *
 * This is only useful if one wants to go from `Stack` -> `StackOpt` -> `RefInfo`.
 * Ultimately, this is just a step on the way to deal with the entire workspace at once.
 */
export type StackOpt = {
	/**
	 * The id of the stack, or null if there is no permanent id.
	 * This can happen if no workspace is known, or even (rare) the workspace
	 * would be out-of-sync with the workspace data that only we can attach.
	 * Ideally there is no catastrophic failure if this is null for one
	 * stack but set for the others.
	 */
	id?: string;
	/**
	 * Information about the branches contained in the stack.
	 */
	heads: StackHeadInfo[];
	/**
	 * The commit hash of the tip of the stack.
	 */
	tip: string;
	/**
	 * Zero-based index for sorting the stacks.
	 */
	order: number;
};

/**
 * Returns the name of the stack.
 *
 * This is the name of the top-most branch in the stack.
 */
export function getStackName(stack: Stack): string {
	if (stack.heads.length === 0) {
		// Should not happen
		throw new Error('Stack has no heads');
	}
	const lastBranch = stack.heads.at(0)!.name;
	return lastBranch;
}

export function getStackBranchNames(stack: Stack): string[] {
	return stack.heads.map((head) => head.name);
}

/** Represents the pushable status for the current stack */
export type PushStatus =
	/**
	 * Can push, but there are no changes to be pushed
	 */
	| 'nothingToPush'
	/**
	 * Can push. This is the case when there are local changes that can be pushed to the remote.
	 */
	| 'unpushedCommits'
	/**
	 * Can push, but requires a force push to the remote because commits were rewritten.
	 */
	| 'unpushedCommitsRequiringForce'
	/**
	 * No commits have been pushed to the remote.
	 */
	| 'completelyUnpushed'
	/**
	 * Every commit is integrated into the base branch.
	 */
	| 'integrated';

export function pushStatusToColor(pushStatus: PushStatus): CellType {
	switch (pushStatus) {
		case 'nothingToPush':
		case 'unpushedCommits':
		case 'unpushedCommitsRequiringForce':
			return 'LocalAndRemote';
		case 'completelyUnpushed':
			return 'LocalOnly';
		case 'integrated':
			return 'Integrated';
	}
}

export function pushStatusToIcon(pushStatus: PushStatus): keyof typeof iconsJson {
	switch (pushStatus) {
		case 'nothingToPush':
		case 'unpushedCommits':
		case 'unpushedCommitsRequiringForce':
			return 'branch-remote';
		case 'completelyUnpushed':
			return 'branch-local';
		case 'integrated':
			return 'branch-remote';
	}
}

export type BranchDetails = {
	/** The name of the branch */
	readonly name: string;
	/** Upstream reference, e.g. `refs/remotes/origin/base-branch-improvements` */
	readonly remoteTrackingBranch: string | null;
	/**
	 * Description of the branch.
	 * Can include arbitrary utf8 data, eg. markdown etc.
	 */
	readonly description: string | null;
	/** The pull(merge) request associated with the branch, or None if no such entity has not been created. */
	readonly prNumber: number | null;
	/** A unique identifier for the GitButler review associated with the branch, if any. */
	readonly reviewId: string | null;
	/**
	 * This is the last commit in the branch, aka the tip of the branch.
	 * If this is the only branch in the stack or the top-most branch, this is the tip of the stack.
	 */
	readonly tip: string;
	/**
	 * This is the base commit from the perspective of this branch.
	 * If the branch is part of a stack and is on top of another branch, this is the head of the branch below it.
	 * If this branch is at the bottom of the stack, this is the merge base of the stack.
	 */
	readonly baseCommit: string;
	/**
	 * The pushable status for the branch
	 */
	pushStatus: PushStatus;
	/**
	 * The last time the branch was updated in Epoch milliseconds
	 */
	lastUpdatedAt: number;
	/**
	 * The authors of the commits in the branch
	 */
	authors: Author[];
	/**
	 * Whether any of the commits contained has conflicts
	 */
	isConflicted: boolean;
	/**
	 *  The commits contained in the branch, excluding the upstream commits.
	 */
	commits: Commit[];
	/**
	 * The commits that are only upstream.
	 */
	upstreamCommits: UpstreamCommit[];
	/** Whether the branch is representing a remote head */
	isRemoteHead: boolean;
};

export type StackDetails = {
	/**
	 * This is the name of the top-most branch, provided by the API for convinience
	 */
	derivedName: string;
	/**
	 * The pushable status for the stack
	 */
	pushStatus: PushStatus;
	/**
	 * The branches that make up the stack
	 */
	branchDetails: BranchDetails[];
	/**
	 * Whether any of the commits contained has conflicts
	 */
	isConflicted: boolean;
};

export function stackRequiresForcePush(stack: StackDetails): boolean {
	return stack.pushStatus === 'unpushedCommitsRequiringForce';
}

export function branchRequiresForcePush(branch: BranchDetails): boolean {
	return branch.pushStatus === 'unpushedCommitsRequiringForce';
}

/**
 * Does the branch or other branch this depends on require a force push?
 *
 * @param branchName The name of the branch to check
 * @param allBranches Complete list of branches in the stack. The order is expected to be child-to-parent
 */
export function partialStackRequestsForcePush(
	branchName: string,
	allBranches: BranchDetails[]
): boolean {
	let foundBranch = false;

	for (const branch of allBranches) {
		if (branch.name === branchName && !foundBranch) {
			foundBranch = true;
		}
		if (!foundBranch) continue;
		if (branchRequiresForcePush(branch)) return true;
	}

	return false;
}

export function stackHasConflicts(stack: StackDetails): boolean {
	return stack.isConflicted;
}

export function branchHasConflicts(branch: BranchDetails): boolean {
	return branch.isConflicted;
}

export function stackHasUnpushedCommits(stack: StackDetails): boolean {
	return requiresPush(stack.pushStatus);
}

export function branchHasUnpushedCommits(branch: BranchDetails): boolean {
	return requiresPush(branch.pushStatus);
}

export function requiresPush(status: PushStatus): boolean {
	return (
		status === 'unpushedCommits' ||
		status === 'unpushedCommitsRequiringForce' ||
		status === 'completelyUnpushed'
	);
}

export type AnchorPosition = 'Above' | 'Below';

export type AtCommitAnchor = {
	type: 'atCommit';
	subject: {
		readonly commit_id: string;
		readonly position: AnchorPosition;
	};
};

export type AtReferenceAnchor = {
	type: 'atReference';
	subject: {
		readonly short_name: string;
		readonly position: AnchorPosition;
	};
};

export type CreateRefAnchor = AtCommitAnchor | AtReferenceAnchor;

export type CreateRefRequest = {
	newName: string;
	anchor: CreateRefAnchor;
};

export type InteractiveIntegrationStep =
	| {
			type: 'skip';
			subject: {
				id: string;
				commitId: string;
			};
	  }
	| {
			type: 'pick';
			subject: {
				id: string;
				commitId: string;
			};
	  }
	| {
			type: 'pickUpstream';
			subject: {
				id: string;
				commitId: string;
				upstreamCommitId: string;
			};
	  }
	| {
			type: 'squash';
			subject: {
				id: string;
				commits: string[];
				message: string | null;
			};
	  };

export type MoveBranchResult = {
	deletedStacks: string[];
	unappliedStacks: string[];
};

export function handleMoveBranchResult(result: MoveBranchResult) {
	if (result.unappliedStacks.length > 0) {
		showToast({
			testId: TestId.StacksUnappliedToast,
			title: 'Heads up: We had to unapply some stacks to move this branch',
			message: `It seems that the branch moved couldn't be applied cleanly alongside your other ${result.unappliedStacks.length} ${result.unappliedStacks.length === 1 ? 'stack' : 'stacks'}.
You can always re-apply them later from the branches page.`
		});
	}
}
