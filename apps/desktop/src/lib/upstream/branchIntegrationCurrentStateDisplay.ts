import type { IntegrationGraphRow } from "$lib/upstream/branchIntegrationView";
import type { InitialBranchIntegration } from "@gitbutler/but-sdk";

export type BranchIntegrationDisplayRow =
	| {
			kind: "collapsedIntegratedLocalSummary";
			hiddenCount: number;
	  }
	| IntegrationGraphRow;

type ContiguousIntegratedCommits = {
	count: number;
	start: number;
	end: number;
};

function identifyContiguousIntegratedCommits(
	initialIntegration: InitialBranchIntegration,
): ContiguousIntegratedCommits {
	let count = 0;
	let start = -1;
	let end = -1;

	for (const [index, commit] of initialIntegration.divergence.localOnly.entries()) {
		if (commit.targetRelation.kind === "notIntegrated") {
			if (count > 0) break;
			continue;
		}

		if (count === 0) start = index;
		count++;
		end = index + 1;
	}

	return {
		count,
		start,
		end,
	};
}

export function buildCurrentStateDisplayRows({
	initialIntegration,
	currentRows,
	showIntegratedLocalCommits,
}: {
	initialIntegration: InitialBranchIntegration;
	currentRows: IntegrationGraphRow[];
	showIntegratedLocalCommits: boolean;
}): BranchIntegrationDisplayRow[] {
	const hiddenCommits = identifyContiguousIntegratedCommits(initialIntegration);
	if (hiddenCommits.count < 2) return currentRows;

	const summaryRow: BranchIntegrationDisplayRow = {
		kind: "collapsedIntegratedLocalSummary",
		hiddenCount: hiddenCommits.count,
	};

	const rowsBeforeSummary = currentRows.slice(0, hiddenCommits.start);
	const rowsAfterSummary = currentRows.slice(hiddenCommits.end);

	if (showIntegratedLocalCommits) {
		return [
			...rowsBeforeSummary,
			summaryRow,
			...currentRows.slice(hiddenCommits.start, hiddenCommits.end),
			...rowsAfterSummary,
		];
	}

	return [...rowsBeforeSummary, summaryRow, ...rowsAfterSummary];
}
