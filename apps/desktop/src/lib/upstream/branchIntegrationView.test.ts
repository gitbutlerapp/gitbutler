import {
	buildCurrentStateGraphRows,
	buildNextStateGraphRows,
} from "$lib/upstream/branchIntegrationView";
import { describe, expect, test } from "vitest";
import type { InitialBranchIntegration, WorkspaceState } from "@gitbutler/but-sdk";

const BRANCH_REF = "refs/heads/feature";

const INITIAL_INTEGRATION: InitialBranchIntegration = {
	integration: {
		mergeBase: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		steps: [],
	},
	divergence: {
		branchRefName: { full: BRANCH_REF },
		upstreamRefName: { full: "refs/remotes/origin/feature" },
		localOnly: [
			{ id: "1111111111111111111111111111111111111111", subject: "Local", refs: ["feature"] },
		],
		upstreamOnly: [
			{
				id: "2222222222222222222222222222222222222222",
				subject: "Upstream",
				refs: ["origin/feature"],
			},
		],
		matched: [{ id: "3333333333333333333333333333333333333333", subject: "Shared", refs: [] }],
		mergeBase: {
			id: "4444444444444444444444444444444444444444",
			subject: "Base",
			refs: [],
		},
	},
};

function encodeRefName(refName: string) {
	return Array.from(new TextEncoder().encode(refName));
}

describe("branchIntegrationView", () => {
	test("builds current-state rows from divergence", () => {
		expect(buildCurrentStateGraphRows(INITIAL_INTEGRATION)).toEqual([
			{
				kind: "commit",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "1111111111111111111111111111111111111111",
					refs: ["feature"],
					subject: "Local",
				},
			},
			{
				kind: "commit",
				leftRail: "|",
				node: "*",
				rightRail: "",
				content: {
					commitId: "2222222222222222222222222222222222222222",
					refs: ["origin/feature"],
					subject: "Upstream",
				},
			},
			{
				kind: "join",
				leftRail: "|",
				node: "",
				rightRail: "/",
			},
			{
				kind: "commit",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "3333333333333333333333333333333333333333",
					refs: [],
					subject: "Shared",
				},
			},
			{
				kind: "commit",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "4444444444444444444444444444444444444444",
					refs: [],
					subject: "Base",
				},
			},
		]);
	});

	test("builds preview rows from workspace state", () => {
		const workspace: WorkspaceState = {
			replacedCommits: {},
			headInfo: {
				workspaceRef: null,
				stacks: [
					{
						id: "stack-1",
						base: "9999999999999999999999999999999999999999",
						segments: [
							{
								refName: {
									fullNameBytes: encodeRefName(BRANCH_REF),
									displayName: "feature",
								},
								remoteTrackingRefName: null,
								commits: [
									{
										id: "5555555555555555555555555555555555555555",
										parentIds: ["6666666666666666666666666666666666666666"],
										message: "Preview title\n\nbody",
										hasConflicts: false,
										state: { type: "LocalOnly" },
										createdAt: 0,
										author: {
											name: "A",
											email: "a@example.com",
											gravatarUrl: "https://example.com/a.png",
										},
										changeId: "",
										gerritReviewUrl: null,
									},
								],
								commitsOnRemote: [],
								commitsOutside: null,
								metadata: null,
								isEntrypoint: true,
								pushStatus: "nothingToPush",
								base: "6666666666666666666666666666666666666666",
							},
							{
								refName: null,
								remoteTrackingRefName: null,
								commits: [
									{
										id: "6666666666666666666666666666666666666666",
										parentIds: [],
										message: "Base title",
										hasConflicts: false,
										state: { type: "Integrated" },
										createdAt: 0,
										author: {
											name: "B",
											email: "b@example.com",
											gravatarUrl: "https://example.com/b.png",
										},
										changeId: "",
										gerritReviewUrl: null,
									},
								],
								commitsOnRemote: [],
								commitsOutside: null,
								metadata: null,
								isEntrypoint: false,
								pushStatus: "integrated",
								base: null,
							},
						],
					},
				],
				target: null,
				isManagedRef: true,
				isManagedCommit: true,
				isEntrypoint: true,
			},
		};

		expect(buildNextStateGraphRows({ workspace, branchRef: BRANCH_REF })).toEqual([
			{
				kind: "commit",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "5555555555555555555555555555555555555555",
					refs: ["feature"],
					subject: "Preview title",
				},
			},
			{
				kind: "commit",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "6666666666666666666666666666666666666666",
					refs: [],
					subject: "Base title",
				},
			},
		]);
	});
});
