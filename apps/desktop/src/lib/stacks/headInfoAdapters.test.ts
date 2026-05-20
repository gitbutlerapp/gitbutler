import { transformWorkspaceDetails } from "$lib/stacks/headInfoAdapters";
import { describe, expect, test } from "vitest";
import type { Author, Commit, RefInfo, Segment, UpstreamCommit } from "@gitbutler/but-sdk";

const encoder = new TextEncoder();

function bytes(value: string): number[] {
	return [...encoder.encode(value)];
}

const author: Author = {
	name: "Ada",
	email: "ada@example.com",
	gravatarUrl: "",
};

const localCommit: Commit = {
	id: "1111111111111111111111111111111111111111",
	parentIds: ["0000000000000000000000000000000000000000"],
	message: "Local commit",
	hasConflicts: true,
	state: { type: "LocalOnly" },
	createdAt: 1000,
	author,
	changeId: "I111",
	gerritReviewUrl: null,
};

const upstreamCommit: UpstreamCommit = {
	id: "2222222222222222222222222222222222222222",
	message: "Remote commit",
	createdAt: 2000,
	author,
	changeId: "I222",
};

function segment(overrides: Partial<Segment> = {}): Segment {
	return {
		refName: {
			fullNameBytes: bytes("refs/heads/feature/top"),
			displayName: "feature/top",
		},
		remoteTrackingRefName: {
			fullNameBytes: bytes("refs/remotes/origin/feature/top"),
			displayName: "feature/top",
			remoteName: "origin",
		},
		commits: [localCommit],
		commitsOnRemote: [upstreamCommit],
		commitsOutside: null,
		metadata: {
			refInfo: {
				createdAt: null,
				updatedAt: { seconds: 123, offset: 0 },
			},
			review: {
				pullRequest: 7,
				reviewId: "review-7",
			},
		},
		isEntrypoint: true,
		pushStatus: "unpushedCommits",
		base: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		...overrides,
	};
}

function refInfo(stacks: RefInfo["stacks"]): RefInfo {
	return {
		workspaceRef: null,
		stacks,
		target: null,
		isManagedRef: true,
		isManagedCommit: true,
		isEntrypoint: true,
	};
}

describe("headInfoAdapters", () => {
	test("maps head_info stacks to legacy stack entries and stack details", () => {
		const result = transformWorkspaceDetails(
			refInfo([
				{
					id: "stack-1",
					base: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
					segments: [segment()],
				},
			]),
		);

		expect(result.stacks.ids).toEqual(["stack-1"]);
		expect(result.stacks.entities["stack-1"]).toMatchObject({
			id: "stack-1",
			tip: localCommit.id,
			order: 0,
			isCheckedOut: true,
			heads: [
				{
					name: "feature/top",
					tip: localCommit.id,
					reviewId: 7,
					isCheckedOut: true,
				},
			],
		});

		const details = result.stackDetails["stack-1"]!;
		expect(details.stackInfo).toMatchObject({
			derivedName: "feature/top",
			pushStatus: "unpushedCommits",
			isConflicted: true,
		});
		expect(details.stackInfo.branchDetails[0]).toMatchObject({
			name: "feature/top",
			reference: "refs/heads/feature/top",
			remoteTrackingBranch: "refs/remotes/origin/feature/top",
			prNumber: 7,
			reviewId: "review-7",
			tip: localCommit.id,
			baseCommit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
			lastUpdatedAt: 123000,
			commits: [localCommit],
			upstreamCommits: [upstreamCommit],
			isRemoteHead: false,
		});
		expect(details.commits.ids).toEqual([localCommit.id]);
		expect(details.upstreamCommits.ids).toEqual([upstreamCommit.id]);
	});

	test("uses the stack base as the tip for empty segments", () => {
		const result = transformWorkspaceDetails(
			refInfo([
				{
					id: "stack-1",
					base: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
					segments: [
						segment({
							commits: [],
							base: null,
						}),
					],
				},
			]),
		);

		expect(result.stacks.entities["stack-1"]?.tip).toBe("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
		expect(result.stackDetails["stack-1"]?.stackInfo.branchDetails[0]?.tip).toBe(
			"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
		);
	});
});
