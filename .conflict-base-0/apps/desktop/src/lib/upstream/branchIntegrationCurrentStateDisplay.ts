import type {
	IntegrationGraphRow,
	IntegrationGraphRowCommit,
	IntegrationGraphRowJoin,
} from "$lib/upstream/branchIntegrationView";
import type { InitialBranchIntegration } from "@gitbutler/but-sdk";

export type BranchIntegrationDisplayLane = "local" | "remote";
export type BranchIntegrationDisplayRailKind = "local" | "integrated";
export type BranchIntegrationDisplayConnectorKind = "local" | "remote" | "integrated";

export type BranchIntegrationDisplayRowSummary = {
	kind: "collapsedIntegratedLocalSummary";
	hiddenCount: number;
	lane: "local";
	showTopConnector: boolean;
	topConnectorKind: BranchIntegrationDisplayConnectorKind;
};

export type BranchIntegrationDisplayRowCommit = IntegrationGraphRowCommit & {
	lane: BranchIntegrationDisplayLane;
	showTopConnector: boolean;
	leftRailKind?: BranchIntegrationDisplayRailKind;
	topConnectorKind: BranchIntegrationDisplayConnectorKind;
};

export type BranchIntegrationDisplayRowJoin = IntegrationGraphRowJoin & {
	leftRailKind?: BranchIntegrationDisplayRailKind;
};

type BranchIntegrationDisplayRowBase =
	| {
			kind: "collapsedIntegratedLocalSummary";
			hiddenCount: number;
	  }
	| IntegrationGraphRow;

export type BranchIntegrationDisplayRow =
	| BranchIntegrationDisplayRowSummary
	| BranchIntegrationDisplayRowCommit
	| BranchIntegrationDisplayRowJoin;

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

function buildVisibleRows({
	currentRows,
	hiddenCommits,
	showIntegratedLocalCommits,
}: {
	currentRows: IntegrationGraphRow[];
	hiddenCommits: ContiguousIntegratedCommits;
	showIntegratedLocalCommits: boolean;
}): BranchIntegrationDisplayRowBase[] {
	if (hiddenCommits.count < 2) return currentRows;

	const summaryRow: BranchIntegrationDisplayRowBase = {
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

function laneForRow(
	row: BranchIntegrationDisplayRowBase,
): BranchIntegrationDisplayLane | undefined {
	if (row.kind === "collapsedIntegratedLocalSummary") return "local";
	if (row.kind === "join") return undefined;
	return row.leftRail === "|" ? "remote" : "local";
}

function localRailKindForRow(
	row: BranchIntegrationDisplayRowBase,
): BranchIntegrationDisplayRailKind | undefined {
	if (laneForRow(row) !== "local") return undefined;
	if (row.kind === "collapsedIntegratedLocalSummary") return "integrated";
	if (row.kind === "join") return undefined;
	return row.commitKind === "integrated" ? "integrated" : "local";
}

function previousLocalRailKind(
	rows: BranchIntegrationDisplayRowBase[],
	index: number,
): BranchIntegrationDisplayRailKind | undefined {
	for (let previousIndex = index - 1; previousIndex >= 0; previousIndex -= 1) {
		const previousRow = rows[previousIndex];
		if (!previousRow) continue;
		const railKind = localRailKindForRow(previousRow);
		if (railKind) return railKind;
	}
	return undefined;
}

function connectorKindForCommitKind(
	commitKind: IntegrationGraphRowCommit["commitKind"],
): BranchIntegrationDisplayConnectorKind {
	return commitKind;
}

function decorateVisibleRows(
	rows: BranchIntegrationDisplayRowBase[],
): BranchIntegrationDisplayRow[] {
	return rows.map((row, index) => {
		const lane = laneForRow(row);
		const previousRow = index > 0 ? rows[index - 1] : undefined;
		const previousLane = previousRow ? laneForRow(previousRow) : undefined;
		const leftRailKind =
			row.kind !== "collapsedIntegratedLocalSummary" && row.leftRail === "|"
				? previousLocalRailKind(rows, index)
				: undefined;
		const mergeBaseTopConnectorKind =
			previousRow?.kind === "join" ? previousLocalRailKind(rows, index) : undefined;
		const showTopConnector =
			mergeBaseTopConnectorKind !== undefined || (lane !== undefined && previousLane === lane);

		if (row.kind === "collapsedIntegratedLocalSummary") {
			return {
				...row,
				lane: "local",
				showTopConnector,
				topConnectorKind: "integrated",
			};
		}

		if (row.kind === "join") {
			return {
				...row,
				leftRailKind,
			};
		}

		return {
			...row,
			lane: lane ?? "local",
			showTopConnector,
			leftRailKind,
			topConnectorKind: mergeBaseTopConnectorKind ?? connectorKindForCommitKind(row.commitKind),
		};
	});
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
	return decorateVisibleRows(
		buildVisibleRows({
			currentRows,
			hiddenCommits,
			showIntegratedLocalCommits,
		}),
	);
}
