import {
	bottomUpdateForStack,
	buildUpstreamIntegrationUpdates,
	deriveUpstreamIntegrationStatuses,
} from "$lib/upstream/types";
import { describe, expect, test } from "vitest";
import type { Author, Commit, RefInfo, Segment, Stack } from "@gitbutler/but-sdk";

const encoder = new TextEncoder();

function bytes(value: string): number[] {
	return [...encoder.encode(value)];
}

const author: Author = {
	name: "Ada",
	email: "ada@example.com",
	gravatarUrl: "",
};

function commit(id: string, hasConflicts = false): Commit {
	return {
		id,
		parentIds: [],
		message: id,
		hasConflicts,
		state: { type: "LocalOnly" },
		createdAt: 1000,
		author,
		changeId: `I${id}`,
		gerritReviewUrl: null,
	};
}

function segment({
	name,
	commits = [],
}: {
	name?: string;
	commits?: Commit[];
} = {}): Segment {
	return {
		refName: name
			? {
					fullNameBytes: bytes(`refs/heads/${name}`),
					displayName: name,
				}
			: null,
		remoteTrackingRefName: null,
		commits,
		commitsOnRemote: [],
		commitsOutside: null,
		metadata: null,
		isEntrypoint: false,
		pushStatus: "unpushedCommits",
		base: null,
	};
}

function stack(segments: Segment[], id = "stack-1"): Stack {
	return {
		id,
		base: null,
		segments,
	};
}

function refInfo(stacks: Stack[]): RefInfo {
	return {
		workspaceRef: null,
		stacks,
		target: null,
		isManagedRef: true,
		isManagedCommit: true,
		isEntrypoint: true,
	};
}

describe("upstream integration types", () => {
	test("builds a rebase bottom update from the bottom-most commit", () => {
		const update = bottomUpdateForStack(
			stack([
				segment({ name: "top", commits: [commit("top")] }),
				segment({ name: "bottom", commits: [commit("middle"), commit("bottom")] }),
			]),
		);

		expect(update).toEqual({
			kind: "rebase",
			selector: {
				type: "commit",
				subject: "bottom",
			},
		});
	});

	test("builds a rebase bottom update from a named empty bottom segment", () => {
		const update = bottomUpdateForStack(stack([segment({ name: "empty" })]));

		expect(update).toEqual({
			kind: "rebase",
			selector: {
				type: "referenceBytes",
				subject: bytes("refs/heads/empty"),
			},
		});
	});

	test("skips stacks without a valid bottom selector", () => {
		expect(buildUpstreamIntegrationUpdates([stack([segment()])])).toEqual([]);
	});

	test("derives branch and stack statuses from preview branch ref names", () => {
		const current = [
			stack(
				[
					segment({ name: "feature/top", commits: [commit("top")] }),
					segment({ name: "feature/middle", commits: [commit("middle")] }),
					segment({ name: "feature/bottom", commits: [commit("bottom")] }),
				],
				"stack-1",
			),
			stack([segment({ name: "feature/clear", commits: [commit("clear")] })], "stack-2"),
			stack([segment({ commits: [commit("unnamed")] })], "stack-3"),
		];
		const preview = refInfo([
			stack([
				segment({ name: "feature/top", commits: [commit("top-preview", true)] }),
				segment({ name: "feature/clear", commits: [commit("clear-preview")] }),
			]),
		]);

		const statuses = deriveUpstreamIntegrationStatuses(current, preview);

		expect(statuses[0]).toMatchObject({
			status: "conflicted",
			fullyIntegrated: false,
			branchStatuses: [
				{ name: "feature/top", status: "conflicted" },
				{ name: "feature/middle", status: "integrated" },
				{ name: "feature/bottom", status: "integrated" },
			],
		});
		expect(statuses[1]).toMatchObject({
			status: "clear",
			fullyIntegrated: false,
			branchStatuses: [{ name: "feature/clear", status: "clear" }],
		});
		expect(statuses[2]).toMatchObject({
			status: "clear",
			fullyIntegrated: false,
			branchStatuses: [{ name: "Unnamed segment", status: "clear" }],
		});
	});

	test("marks a stack integrated only when all branches are integrated", () => {
		const statuses = deriveUpstreamIntegrationStatuses(
			[
				stack([
					segment({ name: "feature/top", commits: [commit("top")] }),
					segment({ name: "feature/bottom", commits: [commit("bottom")] }),
				]),
			],
			refInfo([]),
		);

		expect(statuses[0]).toMatchObject({
			status: "integrated",
			fullyIntegrated: true,
		});
	});
});
