import type { RefInfo, Segment, Stack } from "@gitbutler/but-sdk";

export type WorkspaceUpstreamAction = "rebase" | "merge";
export type WorkspaceUpstreamBranchStatus = "clear" | "integrated" | "conflicted";

export type WorkspaceBottomUpdate = {
	kind: WorkspaceUpstreamAction;
	selector: { type: "commit"; subject: string } | { type: "reference"; subject: string };
};

export type WorkspaceUpstreamSelection = {
	stackKey: string;
	action: WorkspaceUpstreamAction;
	deleteIntegratedBranches: boolean;
};

export type WorkspaceUpstreamRow = {
	stackKey: string;
	stackId: string | null;
	branchNames: string[];
	series: { name: string; status: WorkspaceUpstreamBranchStatus }[];
	canMerge: boolean;
	isFullyIntegrated: boolean;
};

const decoder = new TextDecoder();

function segmentIdentity(segment: Segment): string {
	if (segment.refName) return segment.refName.fullNameBytes.join(",");
	if (segment.commits[0]) return segment.commits[0].id;
	return "missing-segment-identity";
}

function segmentName(segment: Segment): string {
	return segment.refName?.displayName ?? segment.commits[0]?.id ?? "Unknown";
}

function segmentHasConflicts(segment: Segment | undefined): boolean {
	return segment?.commits.some((commit) => commit.hasConflicts) ?? false;
}

function segmentIntegrated(segment: Segment | undefined): boolean {
	return !segment || segment.commits.every((commit) => commit.state.type === "Integrated");
}

function segmentCurrentlyIntegrated(segment: Segment): boolean {
	return segment.commits.length > 0 && segment.commits.every((commit) => commit.state.type === "Integrated");
}

export function getWorkspaceStackKey(stack: Stack, index: number): string {
	return stack.id ?? stack.segments[0]?.refName?.fullNameBytes.join(",") ?? `stack-${index}`;
}

export function canMergeWorkspaceStack(stack: Stack): boolean {
	return stack.segments.length === 1;
}

export function buildWorkspaceUpstreamRows(
	current: RefInfo,
	preview: RefInfo | undefined,
): WorkspaceUpstreamRow[] {
	if (!preview) {
		return current.stacks.map((stack, index) => {
			const series = stack.segments.map((segment) => ({
				name: segmentName(segment),
				status: "clear" as const,
			}));

			return {
				stackKey: getWorkspaceStackKey(stack, index),
				stackId: stack.id,
				branchNames: stack.segments.map(segmentName),
				series,
				canMerge: canMergeWorkspaceStack(stack),
				isFullyIntegrated: false,
			};
		});
	}

	const previewSegments = new Map<string, Segment>();
	for (const stack of preview.stacks) {
		for (const segment of stack.segments) {
			previewSegments.set(segmentIdentity(segment), segment);
		}
	}

	return current.stacks.map((stack, index) => {
		const series = stack.segments.map((segment) => {
			const previewSegment = previewSegments.get(segmentIdentity(segment));
			const currentConflicted = segmentHasConflicts(segment);
			const previewConflicted = segmentHasConflicts(previewSegment);

			let status: WorkspaceUpstreamBranchStatus = "clear";
			if (segmentCurrentlyIntegrated(segment)) {
				status = "integrated";
			} else if (!currentConflicted && previewConflicted) {
				status = "conflicted";
			} else if (segmentIntegrated(previewSegment)) {
				status = "integrated";
			}

			return {
				name: segmentName(segment),
				status,
			};
		});

		return {
			stackKey: getWorkspaceStackKey(stack, index),
			stackId: stack.id,
			branchNames: stack.segments.map(segmentName),
			series,
			canMerge: canMergeWorkspaceStack(stack),
			isFullyIntegrated: series.every((branch) => branch.status === "integrated"),
		};
	});
}

export function getWorkspaceBottomUpdates(
	current: RefInfo,
	selections: Map<string, WorkspaceUpstreamSelection>,
): WorkspaceBottomUpdate[] {
	return current.stacks.reduce<WorkspaceBottomUpdate[]>((updates, stack, index) => {
		const bottomSegment = [...stack.segments].reverse().find((segment) => !segmentCurrentlyIntegrated(segment));
		if (!bottomSegment) return updates;

		const stackKey = getWorkspaceStackKey(stack, index);
		const selection = selections.get(stackKey) ?? {
			stackKey,
			action: "rebase" as const,
			deleteIntegratedBranches: false,
		};

		const bottomCommit = bottomSegment.commits.at(-1);
		if (bottomCommit) {
			updates.push({
				kind: selection.action,
				selector: { type: "commit", subject: bottomCommit.id },
			});
			return updates;
		}

		if (!bottomSegment.refName) {
			throw new Error(`Missing empty-bottom reference for ${stackKey}`);
		}

		updates.push({
			kind: selection.action,
			selector: {
				type: "reference",
				subject: decoder.decode(Uint8Array.from(bottomSegment.refName.fullNameBytes)),
			},
		});
		return updates;
	}, []);
}

export function listIntegratedBranchesToDelete(
	rows: WorkspaceUpstreamRow[],
	selections: Map<string, WorkspaceUpstreamSelection>,
): string[] {
	return rows.flatMap((row) =>
		row.isFullyIntegrated && selections.get(row.stackKey)?.deleteIntegratedBranches
			? row.branchNames
			: [],
	);
}
