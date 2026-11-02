import StackMacros from "$lib/stacks/macros";
import { describe, expect, test, vi } from "vitest";
import type { StackService } from "$lib/stacks/stackService.svelte";
import type { UiState } from "$lib/state/uiState.svelte";

function setup(stackId: string | null = "created-stack") {
	const branchCreateMutation = vi.fn().mockResolvedValue({
		newRef: { displayName: "created-branch" },
		workspace: {
			headInfo: {
				stacks: [
					{ id: "existing-stack", segments: [{ refName: { displayName: "existing-branch" } }] },
					{ id: stackId, segments: [{ refName: { displayName: "created-branch" } }] },
				],
			},
		},
	});
	const createCommitMutation = vi.fn().mockResolvedValue({
		newCommit: "new-commit",
		rejectedChanges: [],
	});
	const selection = { set: vi.fn() };
	const lane = vi.fn(() => ({ selection }));
	const macros = new StackMacros(
		"project-1",
		{
			branchCreateMutation,
			createCommitMutation,
			normalizeBranchName: vi.fn().mockResolvedValue("requested-branch"),
		} as unknown as StackService,
		{ lane } as unknown as UiState,
	);
	return { macros, branchCreateMutation, createCommitMutation, lane, selection };
}

describe("StackMacros.branchChanges", () => {
	test("commits and selects the created branch instead of an existing stack", async () => {
		const { macros, branchCreateMutation, createCommitMutation, lane, selection } = setup();
		await macros.branchChanges({ branchName: "requested branch", worktreeChanges: [] });

		expect(branchCreateMutation).toHaveBeenCalledWith({
			projectId: "project-1",
			newRef: "refs/heads/requested-branch",
			placement: { type: "independent", subject: { order: null } },
		});
		expect(createCommitMutation).toHaveBeenCalledWith(
			expect.objectContaining({
				relativeTo: { type: "reference", subject: "refs/heads/created-branch" },
			}),
		);
		expect(lane).toHaveBeenCalledWith("created-stack");
		expect(selection.set).toHaveBeenCalledWith({
			branchName: "created-branch",
			commitId: "new-commit",
			previewOpen: true,
		});
	});

	test("stops before committing if the created branch has no workspace stack", async () => {
		const { macros, createCommitMutation } = setup(null);
		await expect(macros.createNewStackAndCommit()).rejects.toThrow(
			"Created branch has no workspace stack",
		);
		expect(createCommitMutation).not.toHaveBeenCalled();
	});
});
