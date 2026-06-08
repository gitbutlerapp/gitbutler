import {
	buildCurrentStateGraphRows,
	buildNextStateGraphRows,
} from "$lib/upstream/branchIntegrationView";
import { describe, expect, test } from "vitest";
import type { InitialBranchIntegration, WorkspaceState } from "@gitbutler/but-sdk";

const BRANCH_REF = "refs/heads/feature";
const notIntegrated = { kind: "notIntegrated" } as const;
const author = {
	name: "Test Author",
	email: "author@example.com",
	gravatarUrl: "https://example.com/avatar.png",
};

const INITIAL_INTEGRATION: InitialBranchIntegration = {
	integration: {
		mergeBase: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		firstLocalNotIntegrated: null,
		steps: [],
	},
	divergence: {
		branchRefName: { full: BRANCH_REF },
		upstreamRefName: { full: "refs/remotes/origin/feature" },
		localOnly: [
			{
				id: "1111111111111111111111111111111111111111",
				subject: "Local",
				refs: ["feature"],
				changeId: null,
				createdAt: 0,
				author,
				targetRelation: notIntegrated,
			},
		],
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

function encodeRefName(refName: string) {
	return Array.from(new TextEncoder().encode(refName));
}

describe("branchIntegrationView", () => {
	test("builds current-state rows from divergence", () => {
		expect(buildCurrentStateGraphRows(INITIAL_INTEGRATION)).toEqual([
			{
				kind: "commit",
				commitKind: "local",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "1111111111111111111111111111111111111111",
					refs: ["feature"],
					refDisplays: [{ name: "feature", kind: "local" }],
					subject: "Local",
					changeId: null,
					createdAt: 0,
					author,
					hasConflicts: null,
				},
			},
			{
				kind: "commit",
				commitKind: "remote",
				leftRail: "|",
				node: "*",
				rightRail: "",
				content: {
					commitId: "2222222222222222222222222222222222222222",
					refs: ["origin/feature"],
					refDisplays: [{ name: "origin/feature", kind: "remote" }],
					subject: "Upstream",
					changeId: null,
					createdAt: 0,
					author,
					hasConflicts: null,
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
				commitKind: "remote",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "3333333333333333333333333333333333333333",
					refs: [],
					refDisplays: [],
					subject: "Base",
					changeId: null,
					createdAt: 0,
					author,
					hasConflicts: null,
				},
			},
		]);
	});

	test("styles the local ref on the merge-base when the upstream is ahead", () => {
		const initialIntegration: InitialBranchIntegration = {
			...INITIAL_INTEGRATION,
			divergence: {
				...INITIAL_INTEGRATION.divergence,
				localOnly: [],
				upstreamOnly: [
					{
						id: "cccccccccccccccccccccccccccccccccccccccc",
						subject: "C",
						refs: ["origin/feature"],
						changeId: null,
						createdAt: 0,
						author,
						targetRelation: notIntegrated,
					},
				],
				mergeBase: {
					id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
					subject: "B",
					refs: ["feature"],
					changeId: null,
					createdAt: 0,
					author,
					targetRelation: notIntegrated,
				},
			},
		};

		expect(buildCurrentStateGraphRows(initialIntegration)).toEqual([
			expect.objectContaining({
				kind: "commit",
				commitKind: "remote",
				content: expect.objectContaining({
					commitId: "cccccccccccccccccccccccccccccccccccccccc",
					refs: ["origin/feature"],
					refDisplays: [{ name: "origin/feature", kind: "remote" }],
				}),
			}),
			expect.objectContaining({
				kind: "commit",
				commitKind: "remote",
				content: expect.objectContaining({
					commitId: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
					refs: ["feature"],
					refDisplays: [{ name: "feature", kind: "local" }],
				}),
			}),
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
			expect.objectContaining({
				kind: "commit",
				commitKind: "local",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: expect.objectContaining({
					commitId: "5555555555555555555555555555555555555555",
					refs: ["feature"],
					refDisplays: [{ name: "feature", kind: "local" }],
					subject: "Preview title",
					changeId: "",
					hasConflicts: false,
					author: {
						name: "A",
						email: "a@example.com",
						gravatarUrl: "https://example.com/a.png",
					},
				}),
			}),
			{
				kind: "commit",
				commitKind: "integrated",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "6666666666666666666666666666666666666666",
					refs: [],
					refDisplays: [],
					subject: "Base title",
					changeId: null,
					createdAt: 0,
					hasConflicts: false,
					author: {
						name: "B",
						email: "b@example.com",
						gravatarUrl: "https://example.com/b.png",
					},
				},
			},
		]);
	});
});
