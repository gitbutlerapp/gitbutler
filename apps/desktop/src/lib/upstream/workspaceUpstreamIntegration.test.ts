import {
	buildWorkspaceUpstreamRows,
	canMergeWorkspaceStack,
	getWorkspaceBottomUpdates,
	listIntegratedBranchesToDelete,
	type WorkspaceUpstreamRow,
	type WorkspaceUpstreamSelection,
} from "./workspaceUpstreamIntegration";
import { describe, expect, test } from "vitest";
import type { Commit, RefInfo, Segment, Stack } from "@gitbutler/but-sdk";

function makeCommit(
	id: string,
	options?: { hasConflicts?: boolean; state?: Commit["state"]; integrated?: boolean },
): Commit {
	return {
		id,
		parentIds: [],
		message: id,
		hasConflicts: options?.hasConflicts ?? false,
		state: options?.state ?? (options?.integrated ? { type: "Integrated" } : { type: "LocalOnly" }),
		createdAt: 0,
		author: {
			name: "Test",
			email: "test@example.com",
			gravatarUrl: "",
		},
		changeId: id,
		gerritReviewUrl: null,
	};
}

function makeSegment(
	name: string,
	commitIds: string[],
	options?: { hasConflicts?: boolean; integrated?: boolean },
): Segment {
	return {
		refName: {
			fullNameBytes: Array.from(new TextEncoder().encode(`refs/heads/${name}`)),
			displayName: name,
		},
		remoteTrackingRefName: null,
		commits: commitIds.map((id) =>
			makeCommit(id, {
				hasConflicts: options?.hasConflicts,
				integrated: options?.integrated,
			}),
		),
		commitsOnRemote: [],
		commitsOutside: null,
		metadata: null,
		isEntrypoint: false,
		pushStatus: "nothingToPush",
		base: null,
	};
}

function makeStack(id: string | null, segments: Segment[]): Stack {
	return {
		id,
		base: null,
		segments,
	};
}

function makeHeadInfo(stacks: Stack[]): RefInfo {
	return {
		workspaceRef: null,
		stacks,
		target: null,
		isManagedRef: true,
		isManagedCommit: true,
		isEntrypoint: true,
	};
}

describe("workspaceUpstreamIntegration", () => {
	test("only offers merge for single-segment stacks", () => {
		expect(canMergeWorkspaceStack(makeStack("solo", [makeSegment("feature", ["c2", "c1"])]))).toBe(
			true,
		);

		expect(
			canMergeWorkspaceStack(
				makeStack("stack", [
					makeSegment("feature", ["c4"]),
					makeSegment("base", ["c2", "c1"]),
				]),
			),
		).toBe(false);
	});

	test("builds bottom updates from the bottom commit or empty ref", () => {
		const current = makeHeadInfo([
			makeStack("stack-1", [makeSegment("feature", ["c2", "c1"])]),
			makeStack("stack-2", [makeSegment("empty", [])]),
		]);
		const selections = new Map<string, WorkspaceUpstreamSelection>([
			["stack-1", { stackKey: "stack-1", action: "rebase", deleteIntegratedBranches: false }],
			["stack-2", { stackKey: "stack-2", action: "merge", deleteIntegratedBranches: false }],
		]);

		expect(getWorkspaceBottomUpdates(current, selections)).toEqual([
			{ kind: "rebase", selector: { type: "commit", subject: "c1" } },
			{ kind: "merge", selector: { type: "reference", subject: "refs/heads/empty" } },
		]);
	});

	test("marks only newly introduced branch conflicts", () => {
		const current = makeHeadInfo([
			makeStack("stack-1", [makeSegment("feature", ["c2", "c1"])]),
			makeStack("stack-2", [makeSegment("already-conflicted", ["x2", "x1"], { hasConflicts: true })]),
		]);
		const preview = makeHeadInfo([
			makeStack("stack-1", [makeSegment("feature", ["c2p", "c1p"], { hasConflicts: true })]),
			makeStack("stack-2", [makeSegment("already-conflicted", ["x2p", "x1p"], { hasConflicts: true })]),
		]);

		const rows = buildWorkspaceUpstreamRows(current, preview);

		expect(rows[0]?.series[0]?.status).toBe("conflicted");
		expect(rows[1]?.series[0]?.status).toBe("clear");
	});

	test("keeps rows clear when no preview is available yet", () => {
		const current = makeHeadInfo([makeStack("stack-1", [makeSegment("feature", ["c2", "c1"])])]);

		expect(buildWorkspaceUpstreamRows(current, undefined)[0]?.series[0]?.status).toBe("clear");
	});

	test("treats currently integrated branches as integrated", () => {
		const current = makeHeadInfo([
			makeStack("stack-1", [
				makeSegment("feature", ["c2", "c1"], { integrated: true }),
			]),
		]);
		const preview = makeHeadInfo([
			makeStack("stack-1", [makeSegment("feature", ["c2p", "c1p"], { hasConflicts: true })]),
		]);

		expect(buildWorkspaceUpstreamRows(current, preview)[0]?.series[0]?.status).toBe("integrated");
	});

	test("collects integrated branches chosen for deletion", () => {
		const rows: WorkspaceUpstreamRow[] = [
			{
				stackKey: "stack-1",
				stackId: "stack-1",
				branchNames: ["feature"],
				series: [{ name: "feature", status: "integrated" }],
				canMerge: true,
				isFullyIntegrated: true,
			},
		];
		const selections = new Map<string, WorkspaceUpstreamSelection>([
			["stack-1", { stackKey: "stack-1", action: "rebase", deleteIntegratedBranches: true }],
		]);

		expect(listIntegratedBranchesToDelete(rows, selections)).toEqual(["feature"]);
	});

	test("skips bottom updates for fully integrated stacks", () => {
		const current = makeHeadInfo([
			makeStack("stack-1", [makeSegment("integrated", ["c2", "c1"], { integrated: true })]),
			makeStack("stack-2", [makeSegment("feature", ["x2", "x1"])]),
		]);

		expect(getWorkspaceBottomUpdates(current, new Map())).toEqual([
			{ kind: "rebase", selector: { type: "commit", subject: "x1" } },
		]);
	});

	test("uses the lowest non-integrated segment when integrated parents exist", () => {
		const current = makeHeadInfo([
			makeStack("stack-1", [
				makeSegment("child", ["x2", "x1"]),
				makeSegment("parent", ["c2", "c1"], { integrated: true }),
			]),
		]);

		expect(getWorkspaceBottomUpdates(current, new Map())).toEqual([
			{ kind: "rebase", selector: { type: "commit", subject: "x1" } },
		]);
	});
});
