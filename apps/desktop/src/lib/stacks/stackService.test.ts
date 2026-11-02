import { StackService } from "$lib/stacks/stackService.svelte";
import { describe, expect, test, vi } from "vitest";
import type { BackendApi } from "$lib/state/backendApi";
import type { StackSelection, UiState } from "$lib/state/uiState.svelte";

function makeMockUiState(initialSelection: StackSelection | undefined): {
	uiState: UiState;
	getSelection: () => StackSelection | undefined;
} {
	let selection = initialSelection;
	const laneState = {
		selection: {
			get current() {
				return selection;
			},
			set: (value: StackSelection | undefined) => {
				selection = value;
			},
		},
	};

	return {
		uiState: {
			lane: () => laneState,
		} as unknown as UiState,
		getSelection: () => selection,
	};
}

describe("StackService.uncommit", () => {
	test("clears the selected commit after a successful uncommit", async () => {
		const mutate = vi.fn().mockResolvedValue({
			uncommittedIds: ["commit-1"],
			workspace: {
				headInfo: {
					reference: "refs/heads/branch",
					workspaceChanges: [],
				},
				replacedCommits: {},
			},
		});
		const { uiState, getSelection } = makeMockUiState({
			branchName: "branch",
			commitId: "commit-1",
			previewOpen: true,
		});
		const backendApi = {
			endpoints: {
				uncommit: {
					mutate,
				},
			},
		} as unknown as BackendApi;
		const service = new StackService(backendApi, {} as never, uiState);

		await service.uncommit({
			projectId: "project-1",
			stackId: "stack-1",
			commitIds: ["commit-1"],
		});

		expect(mutate).toHaveBeenCalledWith({
			projectId: "project-1",
			stackId: "stack-1",
			commitIds: ["commit-1"],
		});
		expect(getSelection()).toBeUndefined();
	});
});

describe("StackService.branchRename", () => {
	test("follows the normalized name returned by the backend", () => {
		const useMutation = vi.fn();
		const { uiState, getSelection } = makeMockUiState({ branchName: "old", previewOpen: true });
		Object.assign(uiState, {
			project: () => ({ exclusiveAction: { current: undefined } }),
		});
		const service = new StackService(
			{ endpoints: { branchRename: { useMutation } } } as unknown as BackendApi,
			vi.fn() as never,
			uiState,
		);
		void service.branchRename;
		const options = useMutation.mock.calls[0]![0];
		options.sideEffect(
			{ newRef: { displayName: "my-branch" } },
			{ projectId: "project-1", laneId: "lane-1", branchName: "old", newName: "my branch" },
		);
		expect(getSelection()).toEqual({ branchName: "my-branch", previewOpen: true });
	});
});
