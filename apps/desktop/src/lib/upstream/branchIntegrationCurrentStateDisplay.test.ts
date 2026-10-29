import {
	buildCurrentStateDisplayRows,
	type BranchIntegrationDisplayRow,
} from "$lib/upstream/branchIntegrationCurrentStateDisplay";
import { buildCurrentStateGraphRows } from "$lib/upstream/branchIntegrationView";
import { describe, expect, test } from "vitest";
import type { InitialBranchIntegration } from "@gitbutler/but-sdk";

const BRANCH_REF = "refs/heads/feature";
const notIntegrated = { kind: "notIntegrated" } as const;
const historicallyIntegrated = {
	kind: "historicallyIntegrated",
	targetCommitId: "ffffffffffffffffffffffffffffffffffffffff",
} as const;
const author = {
	name: "Test Author",
	email: "author@example.com",
	gravatarUrl: "https://example.com/avatar.png",
};

function createInitialIntegration(
	localRelations: Array<typeof historicallyIntegrated | typeof notIntegrated>,
) {
	const localOnly = localRelations.map((targetRelation, index) => ({
		id: `${index + 1}`.repeat(40),
		subject: `Local ${index + 1}`,
		refs: index === 0 ? ["feature"] : [],
		changeId: null,
		createdAt: 0,
		author,
		targetRelation,
	}));

	const initialIntegration: InitialBranchIntegration = {
		integration: {
			mergeBase: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
			firstLocalNotIntegrated: null,
			steps: [],
		},
		divergence: {
			branchRefName: { full: BRANCH_REF },
			upstreamRefName: { full: "refs/remotes/origin/feature" },
			localOnly,
			upstreamOnly: [
				{
					id: "2222222222222222222222222222222222222222",
					subject: "Upstream",
					refs: ["origin/feature"],
					changeId: null,
					createdAt: 0,
					author,
					targetRelation: notIntegrated,
				},
			],
			mergeBase: {
				id: "3333333333333333333333333333333333333333",
				subject: "Base",
				refs: [],
				changeId: null,
				createdAt: 0,
				author,
				targetRelation: notIntegrated,
			},
		},
	};

	return initialIntegration;
}

function displayRowsFor({
	localRelations,
	showIntegratedLocalCommits,
}: {
	localRelations: Array<typeof historicallyIntegrated | typeof notIntegrated>;
	showIntegratedLocalCommits: boolean;
}) {
	const initialIntegration = createInitialIntegration(localRelations);
	const currentRows = buildCurrentStateGraphRows(initialIntegration);

	return buildCurrentStateDisplayRows({
		initialIntegration,
		currentRows,
		showIntegratedLocalCommits,
	});
}

function expectCommitRow(
	row: BranchIntegrationDisplayRow,
	{
		commitKind,
		lane,
		showTopConnector,
		leftRailKind,
		topConnectorKind,
	}: {
		commitKind: "local" | "remote" | "integrated";
		lane: "local" | "remote";
		showTopConnector: boolean;
		leftRailKind?: "local" | "integrated";
		topConnectorKind?: "local" | "remote" | "integrated";
	},
) {
	expect(row.kind).toBe("commit");
	if (row.kind !== "commit") return;
	expect(row.commitKind).toBe(commitKind);
	expect(row.lane).toBe(lane);
	expect(row.showTopConnector).toBe(showTopConnector);
	expect(row.leftRailKind).toBe(leftRailKind);
	expect(row.topConnectorKind).toBe(topConnectorKind ?? commitKind);
}

function rowAt(rows: BranchIntegrationDisplayRow[], index: number): BranchIntegrationDisplayRow {
	const row = rows[index];
	expect(row).toBeDefined();
	return row as BranchIntegrationDisplayRow;
}

describe("branchIntegrationCurrentStateDisplay", () => {
	test("collapses the first contiguous integrated local segment by default", () => {
		const rows = displayRowsFor({
			localRelations: [
				notIntegrated,
				historicallyIntegrated,
				historicallyIntegrated,
				notIntegrated,
			],
			showIntegratedLocalCommits: false,
		});

		expect(rows).toHaveLength(6);
		expectCommitRow(rowAt(rows, 0), {
			commitKind: "local",
			lane: "local",
			showTopConnector: false,
		});
		expect(rowAt(rows, 1)).toEqual({
			kind: "collapsedIntegratedLocalSummary",
			hiddenCount: 2,
			lane: "local",
			showTopConnector: true,
			topConnectorKind: "integrated",
		});
		expectCommitRow(rowAt(rows, 2), {
			commitKind: "local",
			lane: "local",
			showTopConnector: true,
		});
		expectCommitRow(rowAt(rows, 3), {
			commitKind: "remote",
			lane: "remote",
			showTopConnector: false,
			leftRailKind: "local",
		});
		expect(rowAt(rows, 4)).toEqual({
			kind: "join",
			leftRail: "|",
			node: "",
			rightRail: "/",
			leftRailKind: "local",
		});
		expectCommitRow(rowAt(rows, 5), {
			commitKind: "remote",
			lane: "local",
			showTopConnector: true,
			topConnectorKind: "local",
		});
	});

	test("collapses the leading integrated local commits by default", () => {
		const rows = displayRowsFor({
			localRelations: [historicallyIntegrated, historicallyIntegrated, notIntegrated],
			showIntegratedLocalCommits: false,
		});

		expect(rows).toHaveLength(5);
		expect(rowAt(rows, 0)).toEqual({
			kind: "collapsedIntegratedLocalSummary",
			hiddenCount: 2,
			lane: "local",
			showTopConnector: false,
			topConnectorKind: "integrated",
		});
		expectCommitRow(rowAt(rows, 1), {
			commitKind: "local",
			lane: "local",
			showTopConnector: true,
		});
		expectCommitRow(rowAt(rows, 2), {
			commitKind: "remote",
			lane: "remote",
			showTopConnector: false,
			leftRailKind: "local",
		});
		expect(rowAt(rows, 3)).toEqual({
			kind: "join",
			leftRail: "|",
			node: "",
			rightRail: "/",
			leftRailKind: "local",
		});
		expectCommitRow(rowAt(rows, 4), {
			commitKind: "remote",
			lane: "local",
			showTopConnector: true,
			topConnectorKind: "local",
		});
	});

	test("shows all rows inline when expanded", () => {
		const rows = displayRowsFor({
			localRelations: [
				notIntegrated,
				historicallyIntegrated,
				historicallyIntegrated,
				notIntegrated,
			],
			showIntegratedLocalCommits: true,
		});

		expect(rows).toHaveLength(8);
		expectCommitRow(rowAt(rows, 0), {
			commitKind: "local",
			lane: "local",
			showTopConnector: false,
		});
		expect(rowAt(rows, 1)).toEqual({
			kind: "collapsedIntegratedLocalSummary",
			hiddenCount: 2,
			lane: "local",
			showTopConnector: true,
			topConnectorKind: "integrated",
		});
		expectCommitRow(rowAt(rows, 2), {
			commitKind: "integrated",
			lane: "local",
			showTopConnector: true,
		});
		expectCommitRow(rowAt(rows, 3), {
			commitKind: "integrated",
			lane: "local",
			showTopConnector: true,
		});
		expectCommitRow(rowAt(rows, 4), {
			commitKind: "local",
			lane: "local",
			showTopConnector: true,
		});
		expectCommitRow(rowAt(rows, 5), {
			commitKind: "remote",
			lane: "remote",
			showTopConnector: false,
			leftRailKind: "local",
		});
		expect(rowAt(rows, 6)).toEqual({
			kind: "join",
			leftRail: "|",
			node: "",
			rightRail: "/",
			leftRailKind: "local",
		});
		expectCommitRow(rowAt(rows, 7), {
			commitKind: "remote",
			lane: "local",
			showTopConnector: true,
			topConnectorKind: "local",
		});
	});

	test("adds display metadata when no rows are collapsed", () => {
		const rows = displayRowsFor({
			localRelations: [historicallyIntegrated, notIntegrated],
			showIntegratedLocalCommits: false,
		});

		expect(rows).toHaveLength(5);
		expectCommitRow(rowAt(rows, 0), {
			commitKind: "integrated",
			lane: "local",
			showTopConnector: false,
		});
		expectCommitRow(rowAt(rows, 1), {
			commitKind: "local",
			lane: "local",
			showTopConnector: true,
		});
		expectCommitRow(rowAt(rows, 2), {
			commitKind: "remote",
			lane: "remote",
			showTopConnector: false,
			leftRailKind: "local",
		});
		expect(rowAt(rows, 3)).toEqual({
			kind: "join",
			leftRail: "|",
			node: "",
			rightRail: "/",
			leftRailKind: "local",
		});
		expectCommitRow(rowAt(rows, 4), {
			commitKind: "remote",
			lane: "local",
			showTopConnector: true,
			topConnectorKind: "local",
		});
	});

	test("uses the last visible local lane row to color remote left rails", () => {
		const rows = displayRowsFor({
			localRelations: [
				notIntegrated,
				historicallyIntegrated,
				historicallyIntegrated,
				notIntegrated,
				historicallyIntegrated,
				historicallyIntegrated,
			],
			showIntegratedLocalCommits: false,
		});

		expect(rows).toHaveLength(8);
		expectCommitRow(rowAt(rows, 4), {
			commitKind: "integrated",
			lane: "local",
			showTopConnector: true,
		});
		expectCommitRow(rowAt(rows, 5), {
			commitKind: "remote",
			lane: "remote",
			showTopConnector: false,
			leftRailKind: "integrated",
		});
		expect(rowAt(rows, 6)).toEqual({
			kind: "join",
			leftRail: "|",
			node: "",
			rightRail: "/",
			leftRailKind: "integrated",
		});
		expectCommitRow(rowAt(rows, 7), {
			commitKind: "remote",
			lane: "local",
			showTopConnector: true,
			topConnectorKind: "integrated",
		});
	});

	test("summary rows count as integrated when coloring remote left rails", () => {
		const rows = displayRowsFor({
			localRelations: [historicallyIntegrated, historicallyIntegrated],
			showIntegratedLocalCommits: false,
		});

		expect(rowAt(rows, 0)).toEqual({
			kind: "collapsedIntegratedLocalSummary",
			hiddenCount: 2,
			lane: "local",
			showTopConnector: false,
			topConnectorKind: "integrated",
		});
		expectCommitRow(rowAt(rows, 1), {
			commitKind: "remote",
			lane: "remote",
			showTopConnector: false,
			leftRailKind: "integrated",
		});
		expect(rowAt(rows, 2)).toEqual({
			kind: "join",
			leftRail: "|",
			node: "",
			rightRail: "/",
			leftRailKind: "integrated",
		});
		expectCommitRow(rowAt(rows, 3), {
			commitKind: "remote",
			lane: "local",
			showTopConnector: true,
			topConnectorKind: "integrated",
		});
	});
});
