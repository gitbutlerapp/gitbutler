import {
	buildCommitPickerOptions,
	buildIntegrationStepDrafts,
	buildInteractiveIntegration,
	changeIntegrationStepDraftKind,
	createDefaultIntegrationStepDraft,
	reorderIntegrationStepDrafts,
	updateIntegrationStepDraftCommit,
	updateIntegrationStepDraftMessage,
} from "$lib/upstream/branchIntegrationEditor";
import { describe, expect, test } from "vitest";
import type { InitialBranchIntegration, InteractiveIntegration } from "@gitbutler/but-sdk";

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

const INITIAL_INTEGRATION: InitialBranchIntegration = {
	integration: {
		mergeBase: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		firstLocalNotIntegrated: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
		steps: [
			{
				kind: "pick",
				commitId: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
			},
			{
				kind: "merge",
				commitId: "cccccccccccccccccccccccccccccccccccccccc",
			},
			{
				kind: "squash",
				commits: [
					"dddddddddddddddddddddddddddddddddddddddd",
					"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
					"ffffffffffffffffffffffffffffffffffffffff",
				],
				message: "combined",
			},
		],
	},
	divergence: {
		branchRefName: { full: "refs/heads/feature" },
		upstreamRefName: { full: "refs/remotes/origin/feature" },
		localOnly: [
			{
				id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
				subject: "Local pick",
				refs: ["feature"],
				changeId: null,
				createdAt: 0,
				author,
				targetRelation: historicallyIntegrated,
			},
		],
		upstreamOnly: [
			{
				id: "cccccccccccccccccccccccccccccccccccccccc",
				subject: "Upstream merge",
				refs: ["origin/feature"],
				changeId: null,
				createdAt: 0,
				author,
				targetRelation: historicallyIntegrated,
			},
			{
				id: "dddddddddddddddddddddddddddddddddddddddd",
				subject: "Upstream squash start",
				refs: [],
				changeId: null,
				createdAt: 0,
				author,
				targetRelation: notIntegrated,
			},
		],
		mergeBase: {
			id: "ffffffffffffffffffffffffffffffffffffffff",
			subject: "Merge base",
			refs: [],
			changeId: null,
			createdAt: 0,
			author,
			targetRelation: notIntegrated,
		},
	},
};

describe("branchIntegrationEditor", () => {
	test("builds commit picker options from editable divergence commits", () => {
		expect(buildCommitPickerOptions(INITIAL_INTEGRATION)).toEqual([
			{
				id: "cccccccccccccccccccccccccccccccccccccccc",
				subject: "Upstream merge",
				refs: ["origin/feature"],
				group: "Upstream",
			},
			{
				id: "dddddddddddddddddddddddddddddddddddddddd",
				subject: "Upstream squash start",
				refs: [],
				group: "Upstream",
			},
		]);
	});

	test("round-trips integration steps through drafts", () => {
		const integration: InteractiveIntegration = buildInteractiveIntegration({
			mergeBase: INITIAL_INTEGRATION.integration.mergeBase,
			firstLocalNotIntegrated: INITIAL_INTEGRATION.integration.firstLocalNotIntegrated,
			steps: buildIntegrationStepDrafts(INITIAL_INTEGRATION.integration),
		});

		expect(integration).toEqual(INITIAL_INTEGRATION.integration);
	});

	test("changes kinds, updates squash messages, and reorders drafts", () => {
		const commitOptions = buildCommitPickerOptions(INITIAL_INTEGRATION);
		const [firstDraft] = buildIntegrationStepDrafts({
			mergeBase: INITIAL_INTEGRATION.integration.mergeBase,
			firstLocalNotIntegrated: null,
			steps: [
				{
					kind: "pick",
					commitId: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
				},
			],
		});
		if (!firstDraft) throw new Error("expected an initial draft");

		const squashDraft = changeIntegrationStepDraftKind({
			step: firstDraft,
			kind: "squash",
			commitOptions,
		});
		expect(squashDraft.kind).toBe("squash");

		const updatedDraft = updateIntegrationStepDraftMessage({
			step: updateIntegrationStepDraftCommit({
				step: squashDraft,
				commitId: "cccccccccccccccccccccccccccccccccccccccc",
				index: 1,
				commitOptions,
			}),
			message: "new message",
		});

		expect(updatedDraft).toEqual({
			id: squashDraft.id,
			kind: "squash",
			commitIds: [
				"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
				"cccccccccccccccccccccccccccccccccccccccc",
			],
			message: "new message",
		});

		const created = createDefaultIntegrationStepDraft(commitOptions);
		const reordered = reorderIntegrationStepDrafts({
			steps: [created, updatedDraft],
			draggedStepId: updatedDraft.id,
			destinationIndex: 0,
		});

		expect(reordered.map((step) => step.id)).toEqual([updatedDraft.id, created.id]);
	});

	test("reorders drafts downward when given the next insertion slot", () => {
		const commitOptions = buildCommitPickerOptions(INITIAL_INTEGRATION);
		const first = createDefaultIntegrationStepDraft(commitOptions);
		const second = {
			...createDefaultIntegrationStepDraft(commitOptions),
			id: "second-step",
		};
		const third = {
			...createDefaultIntegrationStepDraft(commitOptions),
			id: "third-step",
		};

		const reordered = reorderIntegrationStepDrafts({
			steps: [first, second, third],
			draggedStepId: second.id,
			destinationIndex: 3,
		});

		expect(reordered.map((step) => step.id)).toEqual([first.id, third.id, second.id]);
	});
});
