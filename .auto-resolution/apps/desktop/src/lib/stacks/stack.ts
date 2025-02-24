import { showToast } from '$lib/notifications/toasts';
import { TestId } from '@gitbutler/ui';
import type { Workspace, WorkspaceLegacy } from '@gitbutler/core/api';
import type iconsJson from '@gitbutler/ui/data/icons.json';

export type CreateBranchFromBranchOutcome = {
	stackId: string;
	unappliedStacks: string[];
	unappliedStacksShortNames: string[];
};

function stackCount(numStacks: number): string {
	if (numStacks === 1) {
		return 'one stack';
	} else {
		return 'some stacks';
	}
}

function prettyNamedListIfPossible(expectedNames: number, names: string[]): string {
	// It could happen that not all stacks had names, for now we don't deal with that.
	// Also, the old codepath doesn't produce names.
	if (expectedNames !== names.length) {
		return stackCount(expectedNames);
	}
	if (names.length === 0) {
		return '';
	} else if (names.length === 1) {
		return `stack ${names[0]}`;
	} else if (names.length === 2) {
		return `stack ${names[0]} and stack ${names[1]}`;
	}

	const allButLast = names.slice(0, -1);
	const last = names[names.length - 1];

	return `${allButLast.map((n) => `stack ${n}`).join(', ')}, and stack ${last}`;
}

export function handleCreateBranchFromBranchOutcome(outcome: CreateBranchFromBranchOutcome) {
	if (outcome.unappliedStacks.length > 0) {
		showToast({
			testId: TestId.StacksUnappliedToast,
			title: `Heads up: We had to unapply ${stackCount(outcome.unappliedStacks.length)} to apply this one`,
			message: `There were some conflicts detected when applying this branch into your workspace, so we automatically unapplied ${prettyNamedListIfPossible(outcome.unappliedStacks.length, outcome.unappliedStacksShortNames)}.
You can always re-apply them later from the branches page.`
		});
	}
}

/**
 * Return type of Tauri `stacks` command.
 */
export type Stack = WorkspaceLegacy.StackEntry;

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
	heads: WorkspaceLegacy.StackHeadInfo[];
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
export type PushStatus = Workspace.PushStatus;

/**
 * Converts push status directly to a CSS color string.
 */
export function getColorFromPushStatus(pushStatus: PushStatus): string {
	switch (pushStatus) {
		case 'nothingToPush':
		case 'unpushedCommits':
		case 'unpushedCommitsRequiringForce':
			return 'var(--clr-commit-remote)';
		case 'completelyUnpushed':
			return 'var(--clr-commit-local)';
		case 'integrated':
			return 'var(--clr-commit-integrated)';
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

export type BranchDetails = Workspace.BranchDetails;

/** Safely extract the time of the last update to a given branch */
export function branchLastUpdatedAt(branch: BranchDetails): number | null {
	if (branch.lastUpdatedAt === null) return null;
	return Number(branch.lastUpdatedAt);
}

/** Safely extract the date of the last update to a given branch */
export function branchLastUpdatedAtDate(branch: BranchDetails): Date | null {
	const ts = branchLastUpdatedAt(branch);
	if (ts === null) return null;
	return new Date(ts);
}

export type StackDetails = Workspace.StackDetails;

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
