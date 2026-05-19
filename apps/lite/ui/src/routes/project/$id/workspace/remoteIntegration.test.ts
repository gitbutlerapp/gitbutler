import { describe, expect, it } from "vitest";
import type {
	Commit,
	InitialBranchIntegration,
	IntegrationDivergenceDisplay,
	RefInfo,
	Segment,
	WorkspaceState,
} from "@gitbutler/but-sdk";
import { buildNextStateCommitGraphRows, seedIntegrationState } from "./remoteIntegration.ts";

const commit = (id: string, message: string): Commit =>
	({
		id,
		parentIds: [],
		message,
		hasConflicts: false,
		state: { type: "LocalOnly" },
		createdAt: 0,
		author: {
			name: "Test User",
			email: "test@example.com",
			gravatarUrl: null,
			isBot: false,
		},
		changeId: `change-${id}`,
		gerritReviewUrl: null,
	}) as unknown as Commit;

const segment = ({
	branchRef,
	branchName,
	commits,
	base,
}: {
	branchRef: Array<number>;
	branchName: string;
	commits: Array<Commit>;
	base: string | null;
}): Segment =>
	({
		refName: {
			fullNameBytes: branchRef,
			displayName: branchName,
		},
		remoteTrackingRefName: null,
		commits,
		commitsOnRemote: [],
		commitsOutside: null,
		metadata: null,
		isEntrypoint: true,
		pushStatus: "nothingToPush",
		base,
	}) as Segment;

describe("remoteIntegration helpers", () => {
	it("builds the current-state divergence graph rows with refs and merge join", () => {
		const divergence = {
			branchRefName: { full: "refs/heads/feature" },
			upstreamRefName: { full: "refs/remotes/origin/feature" },
			localOnly: [
				{
					id: "2222222222222222222222222222222222222222",
					subject: "local commit",
					refs: ["feature"],
				},
			],
			upstreamOnly: [
				{
					id: "3333333333333333333333333333333333333333",
					subject: "remote commit",
					refs: ["origin/feature"],
				},
			],
			matched: [],
			mergeBase: {
				id: "1111111111111111111111111111111111111111",
				subject: "base commit",
				refs: [],
			},
		} as unknown as IntegrationDivergenceDisplay;

		expect(
			seedIntegrationState({
				integration: {
					mergeBase: "1111111111111111111111111111111111111111",
					steps: [],
				},
				divergence,
			} as InitialBranchIntegration).currentStateRows,
		).toEqual([
			{
				kind: "commit",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "2222222222222222222222222222222222222222",
					refs: ["feature"],
					subject: "local commit",
				},
			},
			{
				kind: "commit",
				leftRail: "|",
				node: "*",
				rightRail: "",
				content: {
					commitId: "3333333333333333333333333333333333333333",
					refs: ["origin/feature"],
					subject: "remote commit",
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
					commitId: "1111111111111111111111111111111111111111",
					refs: [],
					subject: "base commit",
				},
			},
		]);
	});

	it("builds the preview graph rows from the local branch only", () => {
		const branchRef = Array.from(new TextEncoder().encode("refs/heads/feature"));
		const headInfo = {
			workspaceRef: null,
			stacks: [
				{
					id: "stack-1",
					segments: [
						segment({
							branchRef,
							branchName: "feature",
							commits: [
								commit("2222222222222222222222222222222222222222", "tip commit\n"),
								commit("1111111111111111111111111111111111111111", "older commit\n"),
							],
							base: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
						}),
					],
					base: null,
				},
				{
					id: "stack-2",
					segments: [
						segment({
							branchRef: Array.from(new TextEncoder().encode("refs/heads/base")),
							branchName: "base",
							commits: [commit("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "base commit\n")],
							base: null,
						}),
					],
					base: null,
				},
			],
			target: null,
			isManagedRef: true,
			isManagedCommit: true,
			isEntrypoint: true,
		} as RefInfo;

		const workspace = {
			replacedCommits: {},
			headInfo,
		} as WorkspaceState;

		expect(
			buildNextStateCommitGraphRows({
				workspace,
				branchRef: "refs/heads/feature",
			}),
		).toEqual([
			{
				kind: "commit",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "2222222222222222222222222222222222222222",
					refs: ["feature"],
					subject: "tip commit",
				},
			},
			{
				kind: "commit",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "1111111111111111111111111111111111111111",
					refs: [],
					subject: "older commit",
				},
			},
			{
				kind: "commit",
				leftRail: "",
				node: "*",
				rightRail: "",
				content: {
					commitId: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
					refs: [],
					subject: "base commit",
				},
			},
		]);
	});
});
