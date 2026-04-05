import { updateStackSelection } from "$lib/stacks/staleStateUpdaters";
import { describe, expect, test } from "vitest";
import type { StackDetails } from "$lib/stacks/stack";
import type { StackSelection, UiState } from "$lib/state/uiState.svelte";

/**
 * Minimal mock of UiState that tracks the lane selection without Svelte reactivity.
 * Only the parts used by updateStackSelection are implemented.
 */
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
	const uiState = {
		lane: () => laneState,
	} as unknown as UiState;
	return { uiState, getSelection: () => selection };
}

/** Minimal StackDetails with just the fields updateStackSelection reads. */
function makeDetails(
	branches: Array<{
		name: string;
		commits?: Array<{ id: string }>;
		upstreamCommits?: Array<{ id: string }>;
	}>,
): StackDetails {
	return {
		derivedName: "test-stack",
		pushStatus: "nothingToPush",
		isConflicted: false,
		branchDetails: branches.map((b) => ({
			name: b.name,
			commits: (b.commits ?? []) as any,
			upstreamCommits: (b.upstreamCommits ?? []) as any,
			reference: `refs/heads/${b.name}`,
			pushStatus: "nothingToPush",
			isConflicted: false,
			prNumber: undefined,
			reviewId: undefined,
			linkedTo: undefined,
		})),
	} as unknown as StackDetails;
}

const STACK_ID = "stack-1";
const BRANCH = "my-branch";

describe("updateStackSelection", () => {
	describe("no selection", () => {
		test("does nothing when there is no selection", () => {
			const { uiState, getSelection } = makeMockUiState(undefined);
			const details = makeDetails([{ name: BRANCH, commits: [{ id: "sha-a" }] }]);
			updateStackSelection(uiState, STACK_ID, details);
			expect(getSelection()).toBeUndefined();
		});
	});

	describe("branch-level selection (no commitId)", () => {
		test("keeps selection when branch still exists", () => {
			const initial: StackSelection = { branchName: BRANCH, previewOpen: false };
			const { uiState, getSelection } = makeMockUiState(initial);
			const details = makeDetails([{ name: BRANCH }]);
			updateStackSelection(uiState, STACK_ID, details);
			expect(getSelection()).toEqual(initial);
		});

		test("clears selection when branch no longer exists", () => {
			const { uiState, getSelection } = makeMockUiState({
				branchName: "deleted-branch",
				previewOpen: false,
			});
			const details = makeDetails([{ name: BRANCH }]);
			updateStackSelection(uiState, STACK_ID, details);
			expect(getSelection()).toBeUndefined();
		});
	});

	describe("commit-level selection (with commitId)", () => {
		test("keeps selection when commit still exists at the same SHA", () => {
			const initial: StackSelection = {
				branchName: BRANCH,
				commitId: "sha-a",
				previewOpen: false,
			};
			const { uiState, getSelection } = makeMockUiState(initial);
			const details = makeDetails([{ name: BRANCH, commits: [{ id: "sha-a" }] }]);
			updateStackSelection(uiState, STACK_ID, details);
			expect(getSelection()).toEqual(initial);
		});

		test("updates commitId to new SHA when commit is amended (same position, SHA changed)", () => {
			const { uiState, getSelection } = makeMockUiState({
				branchName: BRANCH,
				commitId: "sha-old",
				previewOpen: false,
			});
			const prevDetails = makeDetails([
				{ name: BRANCH, commits: [{ id: "sha-old" }, { id: "sha-b" }] },
			]);
			const nextDetails = makeDetails([
				{ name: BRANCH, commits: [{ id: "sha-new" }, { id: "sha-b" }] },
			]);
			updateStackSelection(uiState, STACK_ID, nextDetails, prevDetails);
			expect(getSelection()).toEqual({
				branchName: BRANCH,
				commitId: "sha-new",
				previewOpen: false,
			});
		});

		test("updates commitId when a non-tip commit is amended (selected descendant also gets new SHA)", () => {
			// Commits are ordered [top=index 0 (HEAD/newest), middle=index 1, bottom=index 2 (oldest)].
			// Amending "middle" rebases "top" onto it, giving "top" a new SHA.
			// "bottom" is an ancestor of "middle" and keeps its SHA unchanged.
			const { uiState, getSelection } = makeMockUiState({
				branchName: BRANCH,
				commitId: "sha-top", // selected commit is a descendant of the amended commit
				previewOpen: false,
			});
			const prevDetails = makeDetails([
				{ name: BRANCH, commits: [{ id: "sha-top" }, { id: "sha-middle" }, { id: "sha-bottom" }] },
			]);
			const nextDetails = makeDetails([
				{
					name: BRANCH,
					// top and middle get new SHAs; bottom (ancestor) is unchanged
					commits: [{ id: "sha-top-new" }, { id: "sha-middle-new" }, { id: "sha-bottom" }],
				},
			]);
			updateStackSelection(uiState, STACK_ID, nextDetails, prevDetails);
			expect(getSelection()).toMatchObject({ commitId: "sha-top-new" });
		});

		test("clears commitId when the commit is deleted (branch has fewer commits)", () => {
			const { uiState, getSelection } = makeMockUiState({
				branchName: BRANCH,
				commitId: "sha-deleted",
				previewOpen: false,
			});
			const prevDetails = makeDetails([
				{ name: BRANCH, commits: [{ id: "sha-deleted" }, { id: "sha-b" }] },
			]);
			// sha-deleted is gone and the branch now has only 1 commit
			const nextDetails = makeDetails([{ name: BRANCH, commits: [{ id: "sha-b" }] }]);
			updateStackSelection(uiState, STACK_ID, nextDetails, prevDetails);
			expect(getSelection()).toEqual({ branchName: BRANCH, previewOpen: false });
		});

		test("clears commitId on first load when the commit is not found (no prevDetails)", () => {
			const { uiState, getSelection } = makeMockUiState({
				branchName: BRANCH,
				commitId: "sha-stale",
				previewOpen: false,
			});
			const nextDetails = makeDetails([{ name: BRANCH, commits: [{ id: "sha-a" }] }]);
			updateStackSelection(uiState, STACK_ID, nextDetails, undefined);
			expect(getSelection()).toEqual({ branchName: BRANCH, previewOpen: false });
		});
	});

	describe("upstream commit selection", () => {
		test("keeps selection when upstream commit still exists", () => {
			const initial: StackSelection = {
				branchName: BRANCH,
				commitId: "upstream-sha",
				upstream: true,
				previewOpen: false,
			};
			const { uiState, getSelection } = makeMockUiState(initial);
			const details = makeDetails([{ name: BRANCH, upstreamCommits: [{ id: "upstream-sha" }] }]);
			updateStackSelection(uiState, STACK_ID, details);
			expect(getSelection()).toEqual(initial);
		});

		test("clears commitId when upstream commit is gone", () => {
			const { uiState, getSelection } = makeMockUiState({
				branchName: BRANCH,
				commitId: "upstream-gone",
				upstream: true,
				previewOpen: false,
			});
			const details = makeDetails([{ name: BRANCH, upstreamCommits: [{ id: "upstream-other" }] }]);
			updateStackSelection(uiState, STACK_ID, details);
			expect(getSelection()).toEqual({ branchName: BRANCH, previewOpen: false });
		});
	});
});
