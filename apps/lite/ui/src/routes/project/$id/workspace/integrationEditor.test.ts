import { describe, expect, it } from "vitest";
import type { InitialBranchIntegration } from "@gitbutler/but-sdk";
import {
	buildCommitPickerOptions,
	buildIntegrationStepDrafts,
	buildInteractiveIntegration,
	reorderIntegrationStepDrafts,
} from "./integrationEditor.ts";

const initialIntegration = {
	integration: {
		mergeBase: "1111111111111111111111111111111111111111",
		steps: [
			{ kind: "pick", commit_id: "2222222222222222222222222222222222222222" },
			{
				kind: "squash",
				commits: [
					"3333333333333333333333333333333333333333",
					"4444444444444444444444444444444444444444",
				],
				message: "combined",
			},
			{ kind: "merge", commit_id: "5555555555555555555555555555555555555555" },
		],
	},
	divergence: {
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
				id: "5555555555555555555555555555555555555555",
				subject: "remote commit",
				refs: ["origin/feature"],
			},
		],
		matched: [
			{
				id: "3333333333333333333333333333333333333333",
				subject: "shared commit",
				refs: [],
			},
			{
				id: "4444444444444444444444444444444444444444",
				subject: "shared commit two",
				refs: [],
			},
		],
		mergeBase: {
			id: "1111111111111111111111111111111111111111",
			subject: "base commit",
			refs: [],
		},
	},
} as InitialBranchIntegration;

describe("integrationEditor helpers", () => {
	it("builds commit picker options from the current state without the merge base", () => {
		expect(buildCommitPickerOptions(initialIntegration)).toEqual([
			{
				id: "2222222222222222222222222222222222222222",
				subject: "local commit",
				refs: ["feature"],
				group: "Local",
			},
			{
				id: "5555555555555555555555555555555555555555",
				subject: "remote commit",
				refs: ["origin/feature"],
				group: "Upstream",
			},
			{
				id: "3333333333333333333333333333333333333333",
				subject: "shared commit",
				refs: [],
				group: "Shared",
			},
			{
				id: "4444444444444444444444444444444444444444",
				subject: "shared commit two",
				refs: [],
				group: "Shared",
			},
		]);
	});

	it("round-trips integration steps through drafts", () => {
		const drafts = buildIntegrationStepDrafts(initialIntegration.integration);
		expect(
			buildInteractiveIntegration({
				mergeBase: initialIntegration.integration.mergeBase,
				steps: drafts,
			}),
		).toEqual(initialIntegration.integration);
	});

	it("reorders dragged steps by destination slot", () => {
		const drafts = buildIntegrationStepDrafts(initialIntegration.integration);
		const [first, second, third] = drafts;
		if (!first || !second || !third) throw new Error("Expected three drafts");

		expect(
			reorderIntegrationStepDrafts({
				steps: drafts,
				draggedStepId: third.id,
				destinationIndex: 1,
			}).map((step) => step.id),
		).toEqual([first.id, third.id, second.id]);
	});
});
