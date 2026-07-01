import { showToast } from "$lib/notifications/toasts";
import { TestId } from "@gitbutler/ui";
import type { BranchIconName } from "$lib/branches/branchIcon";
import type { DropResult } from "$lib/dragging/dropResult";
import type { PushStatus, Segment, Stack as RefInfoStack } from "@gitbutler/but-sdk";

export type CreateBranchFromBranchOutcome = {
	stackId: string;
	unappliedStacks: string[];
	unappliedStacksShortNames: string[];
};

function stackCount(numStacks: number): string {
	if (numStacks === 1) {
		return "one stack";
	} else {
		return "some stacks";
	}
}

function prettyNamedListIfPossible(expectedNames: number, names: string[]): string {
	// It could happen that not all stacks had names, for now we don't deal with that.
	// Also, the old codepath doesn't produce names.
	if (expectedNames !== names.length) {
		return stackCount(expectedNames);
	}
	if (names.length === 0) {
		return "";
	} else if (names.length === 1) {
		return `stack ${names[0]}`;
	} else if (names.length === 2) {
		return `stack ${names[0]} and stack ${names[1]}`;
	}

	const allButLast = names.slice(0, -1);
	const last = names[names.length - 1];

	return `${allButLast.map((n) => `stack ${n}`).join(", ")}, and stack ${last}`;
}

export function handleCreateBranchFromBranchOutcome(outcome: CreateBranchFromBranchOutcome) {
	if (outcome.unappliedStacks.length > 0) {
		showToast({
			testId: TestId.StacksUnappliedToast,
			title: `Heads up: We had to unapply ${stackCount(outcome.unappliedStacks.length)} to apply this one`,
			message: `There were some conflicts detected when applying this branch into your workspace, so we automatically unapplied ${prettyNamedListIfPossible(outcome.unappliedStacks.length, outcome.unappliedStacksShortNames)}.
You can always re-apply them later from the branches page.`,
		});
	}
}

export type Stack = RefInfoStack;

export type GerritPushFlag =
	| { type: "wip" }
	| { type: "ready" }
	| { type: "private" }
	| { type: "hashtag"; subject: string }
	| { type: "topic"; subject: string };

/**
 * Returns the name of the stack.
 *
 * This is the name of the top-most branch in the stack.
 */
export function getStackName(stack: Stack): string {
	const firstSegment = stack.segments.at(0);
	if (!firstSegment?.refName) {
		return "Unnamed segment";
	}
	return firstSegment.refName.displayName;
}

export function getStackBranchNames(stack: Stack): string[] {
	return stack.segments.map((segment) => {
		if (!segment.refName) {
			return "Unnamed segment";
		}
		return segment.refName.displayName;
	});
}

/**
 * Converts push status directly to a CSS color string.
 */
export function getColorFromPushStatus(pushStatus: PushStatus): string {
	switch (pushStatus) {
		case "nothingToPush":
		case "unpushedCommits":
		case "unpushedCommitsRequiringForce":
			return "var(--commit-remote)";
		case "completelyUnpushed":
			return "var(--commit-local)";
		case "integrated":
			return "var(--commit-integrated)";
	}
}

export function pushStatusToIcon(pushStatus: PushStatus): BranchIconName {
	switch (pushStatus) {
		case "nothingToPush":
		case "unpushedCommits":
		case "unpushedCommitsRequiringForce":
			return "branch";
		case "completelyUnpushed":
			return "branch-local";
		case "integrated":
			return "branch";
	}
}

export function stackRequiresForcePush(stack: Stack): boolean {
	return stack.segments.at(0)?.pushStatus === "unpushedCommitsRequiringForce";
}

export function branchRequiresForcePush(branch: Segment): boolean {
	return branch.pushStatus === "unpushedCommitsRequiringForce";
}

export function stackHasConflicts(stack: Stack): boolean {
	return stack.segments.at(0)?.commits.some((commit) => commit.hasConflicts) ?? false;
}

export function branchHasConflicts(branch: Segment): boolean {
	return branch.commits.some((commit) => commit.hasConflicts);
}

export function stackHasUnpushedCommits(stack: Stack): boolean {
	const pushStatus = stack.segments.at(0)?.pushStatus;
	return pushStatus ? requiresPush(pushStatus) : false;
}

export function branchHasUnpushedCommits(branch: Segment): boolean {
	return requiresPush(branch.pushStatus);
}

export function requiresPush(status: PushStatus): boolean {
	return (
		status === "unpushedCommits" ||
		status === "unpushedCommitsRequiringForce" ||
		status === "completelyUnpushed"
	);
}

export type AnchorPosition = "Above" | "Below";

export type AtCommitAnchor = {
	type: "atCommit";
	subject: {
		readonly commit_id: string;
		readonly position: AnchorPosition;
	};
};

export type AtSegmentAnchor = {
	type: "atSegment";
	subject: {
		readonly short_name: string;
		readonly position: AnchorPosition;
	};
};

/**
 * Unlike `AtSegmentAnchor`, the new reference always points at the same commit
 * as the anchor reference - `position` only determines their ordering.
 */
export type AtReferenceAnchor = {
	type: "atReference";
	subject: {
		readonly short_name: string;
		readonly position: AnchorPosition;
	};
};

export type CreateRefAnchor = AtCommitAnchor | AtSegmentAnchor | AtReferenceAnchor;

export type CreateRefRequest = {
	newName: string;
	anchor: CreateRefAnchor;
};

/**
 * Converts an unapplied-stack count into a `DropResult` warning if stacks were unapplied.
 */
export function toMoveBranchWarning(unappliedStackCount: number): DropResult | undefined {
	if (unappliedStackCount === 0) return undefined;
	return {
		type: "warning",
		title: "Heads up: We had to unapply some stacks to move this branch",
		message: `It seems that the branch moved couldn't be applied cleanly alongside your other ${unappliedStackCount} ${unappliedStackCount === 1 ? "stack" : "stacks"}.
You can always re-apply them later from the branches page.`,
		testId: TestId.StacksUnappliedToast,
	};
}
