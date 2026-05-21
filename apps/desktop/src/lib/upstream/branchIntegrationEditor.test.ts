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

const INITIAL_INTEGRATION: InitialBranchIntegration = {
	integration: {
		mergeBase: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		steps: [
			{
				kind: "pick",
				commit_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
			},
			{
				kind: "merge",
				commit_id: "cccccccccccccccccccccccccccccccccccccccc",
			},
			{
				kind: "squash",
				commits: [
					"dddddddddddddddddddddddddddddddddddddddd",
					"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
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
			},
		],
		upstreamOnly: [
			{
				id: "cccccccccccccccccccccccccccccccccccccccc",
				subject: "Upstream merge",
				refs: ["origin/feature"],
			},
		],
		matched: [
			{
				id: "dddddddddddddddddddddddddddddddddddddddd",
				subject: "Shared ancestor",
				refs: [],
			},
			{
				id: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
				subject: "Shared tip",
				refs: [],
			},
		],
		mergeBase: {
			id: "ffffffffffffffffffffffffffffffffffffffff",
			subject: "Merge base",
			refs: [],
		},
	},
};

describe("branchIntegrationEditor", () => {
	test("builds commit picker options from divergence buckets", () => {
		expect(buildCommitPickerOptions(INITIAL_INTEGRATION)).toEqual([
			{
				id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
				subject: "Local pick",
				refs: ["feature"],
				group: "Local",
			},
			{
				id: "cccccccccccccccccccccccccccccccccccccccc",
				subject: "Upstream merge",
				refs: ["origin/feature"],
				group: "Upstream",
			},
			{
				id: "dddddddddddddddddddddddddddddddddddddddd",
				subject: "Shared ancestor",
				refs: [],
				group: "Shared",
			},
			{
				id: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
				subject: "Shared tip",
				refs: [],
				group: "Shared",
			},
		]);
	});

	test("round-trips integration steps through drafts", () => {
		const integration: InteractiveIntegration = buildInteractiveIntegration({
			mergeBase: INITIAL_INTEGRATION.integration.mergeBase,
			steps: buildIntegrationStepDrafts(INITIAL_INTEGRATION.integration),
		});

		expect(integration).toEqual(INITIAL_INTEGRATION.integration);
	});

	test("changes kinds, updates squash messages, and reorders drafts", () => {
		const commitOptions = buildCommitPickerOptions(INITIAL_INTEGRATION);
		const [firstDraft] = buildIntegrationStepDrafts({
			mergeBase: INITIAL_INTEGRATION.integration.mergeBase,
			steps: [
				{
					kind: "pick",
					commit_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
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
});
