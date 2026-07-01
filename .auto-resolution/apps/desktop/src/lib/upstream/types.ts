import { getStackName, type Stack } from "$lib/stacks/stack";
import type { BottomUpdate, RefInfo, Segment } from "@gitbutler/but-sdk";

export type UpstreamIntegrationDisplayStatus = "integrated" | "conflicted" | "clear";

export type UpstreamIntegrationBranchStatus = {
	name: string;
	status: UpstreamIntegrationDisplayStatus;
};

export type UpstreamIntegrationStackStatus = {
	stack: Stack;
	status: UpstreamIntegrationDisplayStatus;
	branchStatuses: UpstreamIntegrationBranchStatus[];
	fullyIntegrated: boolean;
};

export type UpstreamIntegrationStatuses = {
	subject: UpstreamIntegrationStackStatus[];
	updates: BottomUpdate[];
	worktreeConflicts: string[];
};

const textDecoder = new TextDecoder();

function refNameKey(segment: Segment): string | undefined {
	if (!segment.refName) return;
	return textDecoder.decode(Uint8Array.from(segment.refName.fullNameBytes));
}

function branchDisplayName(segment: Segment): string {
	return segment.refName?.displayName ?? "Unnamed segment";
}

function bottomSegment(stack: Stack): Segment | undefined {
	return stack.segments.at(-1);
}

export function bottomUpdateForStack(stack: Stack): BottomUpdate | undefined {
	const segment = bottomSegment(stack);
	if (!segment) return;

	const bottomCommit = segment.commits.at(-1);
	if (bottomCommit) {
		return {
			kind: "rebase",
			selector: {
				type: "commit",
				subject: bottomCommit.id,
			},
		};
	}

	if (segment.refName) {
		return {
			kind: "rebase",
			selector: {
				type: "referenceBytes",
				subject: segment.refName.fullNameBytes,
			},
		};
	}
}

/**
 * Create the bottom updates for the given stacks.
 *
 * We create the bottom updates by figuring out the bottom-most commit per stack falling-back to the
 * bottom-most segment for stacks with empty branches at the bottom.
 * @param stacks - The stacks to map into bottom updates.
 * @returns - An array of bottom updates.
 */
export function buildUpstreamIntegrationUpdates(stacks: Stack[]): BottomUpdate[] {
	return stacks.map(bottomUpdateForStack).filter((update): update is BottomUpdate => !!update);
}

function previewSegmentsByRefName(previewHeadInfo: RefInfo): Map<string, Segment> {
	const segments = new Map<string, Segment>();

	for (const stack of previewHeadInfo.stacks) {
		for (const segment of stack.segments) {
			const key = refNameKey(segment);
			if (key) segments.set(key, segment);
		}
	}

	return segments;
}

/**
 * Derive the branch status from the map of preview branches.
 *
 * We match the actual branch to the one in the preview.
 * - If the branch is anonymous, we consider it clear.
 * - If the branch is not part of the preview, we consider it integrated.
 * - If the matched branch contains conflicts, we consider it conflicted.
 * - Otherwise, the branch is considered clear.
 */
function deriveBranchStatus(
	segment: Segment,
	previewSegments: Map<string, Segment>,
): UpstreamIntegrationBranchStatus {
	const key = refNameKey(segment);
	if (!key) {
		return {
			name: branchDisplayName(segment),
			status: "clear",
		};
	}

	const previewSegment = previewSegments.get(key);
	if (!previewSegment) {
		return {
			name: branchDisplayName(segment),
			status: "integrated",
		};
	}

	const hasConflicts = previewSegment.commits.some((commit) => commit.hasConflicts);

	return {
		name: branchDisplayName(segment),
		status: hasConflicts ? "conflicted" : "clear",
	};
}

/**
 * Determine the status of a stack, by the status of its branches.
 */
function deriveStackStatus(
	branchStatuses: UpstreamIntegrationBranchStatus[],
): UpstreamIntegrationDisplayStatus {
	if (branchStatuses.every((branchStatus) => branchStatus.status === "integrated")) {
		return "integrated";
	}

	if (branchStatuses.some((branchStatus) => branchStatus.status === "conflicted")) {
		return "conflicted";
	}

	return "clear";
}

/**
 * Compare a set of current stacks against a preview in order to determine the upcoming statuses.
 *
 * We compare the branches of each stack to their preview counterpart in order to determine the status
 * of the branches, and based on that we determine the status of the stacks.
 *
 * @param currentStacks - The set of stacks we want to get statuses for.
 * @param previewHeadInfo - The preview which will determine the statuses for the given stacks.
 * @returns - An array of stack statuses.
 */
export function deriveUpstreamIntegrationStatuses(
	currentStacks: Stack[],
	previewHeadInfo: RefInfo,
): UpstreamIntegrationStackStatus[] {
	const previewSegments = previewSegmentsByRefName(previewHeadInfo);

	return currentStacks.map((stack) => {
		const branchStatuses = stack.segments.map((segment) =>
			deriveBranchStatus(segment, previewSegments),
		);
		const status = deriveStackStatus(branchStatuses);

		return {
			stack,
			status,
			branchStatuses,
			fullyIntegrated: status === "integrated",
		};
	});
}

export function clearUpstreamIntegrationStatuses(
	currentStacks: Stack[],
): UpstreamIntegrationStackStatus[] {
	return currentStacks.map((stack) => {
		const branchStatuses = stack.segments.map((segment) => ({
			name: branchDisplayName(segment),
			status: "clear" as const,
		}));

		return {
			stack,
			status: "clear",
			branchStatuses,
			fullyIntegrated: false,
		};
	});
}

export function sortUpstreamIntegrationStatus(
	a: UpstreamIntegrationStackStatus,
	b: UpstreamIntegrationStackStatus,
): number {
	if (a.fullyIntegrated === b.fullyIntegrated) {
		return getStackName(a.stack).localeCompare(getStackName(b.stack));
	}

	return a.fullyIntegrated ? 1 : -1;
}
