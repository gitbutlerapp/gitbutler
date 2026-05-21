import { buildCurrentStateDisplayRows } from "$lib/upstream/branchIntegrationCurrentStateDisplay";
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

describe("branchIntegrationCurrentStateDisplay", () => {
	test("collapses the first contiguous integrated local segment by default", () => {
		const initialIntegration = createInitialIntegration([
			notIntegrated,
			historicallyIntegrated,
			historicallyIntegrated,
			notIntegrated,
		]);
		const currentRows = buildCurrentStateGraphRows(initialIntegration);

		expect(
			buildCurrentStateDisplayRows({
				initialIntegration,
				currentRows,
				showIntegratedLocalCommits: false,
			}),
		).toEqual([
			currentRows[0],
			{
				kind: "collapsedIntegratedLocalSummary",
				hiddenCount: 2,
			},
			currentRows[3],
			currentRows[4],
			currentRows[5],
			currentRows[6],
		]);
	});

	test("collapses the leading integrated local commits by default", () => {
		const initialIntegration = createInitialIntegration([
			historicallyIntegrated,
			historicallyIntegrated,
			notIntegrated,
		]);
		const currentRows = buildCurrentStateGraphRows(initialIntegration);

		expect(
			buildCurrentStateDisplayRows({
				initialIntegration,
				currentRows,
				showIntegratedLocalCommits: false,
			}),
		).toEqual([
			{
				kind: "collapsedIntegratedLocalSummary",
				hiddenCount: 2,
			},
			currentRows[2],
			currentRows[3],
			currentRows[4],
			currentRows[5],
		]);
	});

	test("shows all rows inline when expanded", () => {
		const initialIntegration = createInitialIntegration([
			notIntegrated,
			historicallyIntegrated,
			historicallyIntegrated,
			notIntegrated,
		]);
		const currentRows = buildCurrentStateGraphRows(initialIntegration);

		expect(
			buildCurrentStateDisplayRows({
				initialIntegration,
				currentRows,
				showIntegratedLocalCommits: true,
			}),
		).toEqual([
			currentRows[0],
			{
				kind: "collapsedIntegratedLocalSummary",
				hiddenCount: 2,
			},
			currentRows[1],
			currentRows[2],
			currentRows[3],
			currentRows[4],
			currentRows[5],
			currentRows[6],
		]);
	});

	test("does not collapse a single integrated local commit", () => {
		const initialIntegration = createInitialIntegration([historicallyIntegrated, notIntegrated]);
		const currentRows = buildCurrentStateGraphRows(initialIntegration);

		expect(
			buildCurrentStateDisplayRows({
				initialIntegration,
				currentRows,
				showIntegratedLocalCommits: false,
			}),
		).toEqual(currentRows);
	});

	test("does not collapse a later single integrated segment", () => {
		const initialIntegration = createInitialIntegration([
			notIntegrated,
			historicallyIntegrated,
			notIntegrated,
		]);
		const currentRows = buildCurrentStateGraphRows(initialIntegration);

		expect(
			buildCurrentStateDisplayRows({
				initialIntegration,
				currentRows,
				showIntegratedLocalCommits: false,
			}),
		).toEqual(currentRows);
	});

	test("does not collapse a second integrated segment after the first one", () => {
		const initialIntegration = createInitialIntegration([
			notIntegrated,
			historicallyIntegrated,
			historicallyIntegrated,
			notIntegrated,
			historicallyIntegrated,
			historicallyIntegrated,
		]);
		const currentRows = buildCurrentStateGraphRows(initialIntegration);

		expect(
			buildCurrentStateDisplayRows({
				initialIntegration,
				currentRows,
				showIntegratedLocalCommits: false,
			}),
		).toEqual([
			currentRows[0],
			{
				kind: "collapsedIntegratedLocalSummary",
				hiddenCount: 2,
			},
			currentRows[3],
			currentRows[4],
			currentRows[5],
			currentRows[6],
			currentRows[7],
			currentRows[8],
		]);
	});
});
