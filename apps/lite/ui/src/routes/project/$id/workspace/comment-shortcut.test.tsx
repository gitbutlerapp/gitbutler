/** @vitest-environment jsdom */

import type { ForgeReview, WorktreeChanges } from "@gitbutler/but-sdk";
import { getHotkeyManager } from "@tanstack/react-hotkeys";
import type * as ReactQuery from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

declare global {
	var IS_REACT_ACT_ENVIRONMENT: boolean | undefined;
}

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
globalThis.window.lite = { platform: "linux" } as typeof window.lite;

const mutations = vi.hoisted(() => ({ commit: vi.fn(), comment: vi.fn() }));

vi.mock("#ui/api/mutations.ts", () => ({
	useBranchCreate: () => ({ isPending: false, mutate: vi.fn() }),
	useCommitCreate: () => ({ isPending: false, mutate: mutations.commit }),
	useCreateReviewComment: () => ({ mutate: mutations.comment }),
	useGenerateCommitMessage: () => ({ isPending: false, mutate: vi.fn() }),
	useUploadFiles: () => vi.fn(),
}));
vi.mock("#ui/commit.ts", () => ({
	draftCommitMessageQueryOptions: () => ({}),
	usePersistDraftCommitMessage: () => ({ mutate: vi.fn() }),
}));
vi.mock("#ui/projects/state.ts", () => ({
	projectSlice: {
		selectors: {
			selectCheckedUncommittedFilePaths: () => new Set(),
			selectPendingOperation: () => ({ _tag: "None" }),
		},
	},
}));
vi.mock("#ui/store.ts", () => ({
	useAppSelector: (selector: (state: object) => unknown) => selector({}),
	useAppStore: () => ({ getState: () => ({}) }),
}));
vi.mock("#ui/use-cursor.ts", () => ({ setCursor: vi.fn() }));
vi.mock("#ui/review-arrival.tsx", () => ({
	FreshBadge: () => null,
	RegisterFreshItems: () => null,
}));
vi.mock("@tanstack/react-query", async (importOriginal) => ({
	...(await importOriginal<typeof ReactQuery>()),
	useIsMutating: () => 0,
	useQuery: () => ({ data: undefined, isPending: false }),
}));

const [{ CommitForm }, { PullRequestComments }] = await Promise.all([
	import("./CommitForm.tsx"),
	import("./PullRequestComments.tsx"),
]);

const review = {
	number: 7,
	sourceBranch: "review-branch",
	createdAt: null,
} as ForgeReview;
const worktreeChanges = {
	changes: [],
} as unknown as WorktreeChanges;

const Fixture = () => (
	<>
		<CommitForm
			projectId="project-id"
			commitTarget={{
				label: "main",
				address: { _tag: "Branch", branchRef: [] },
				relativeTo: { type: "referenceBytes", subject: [] },
			}}
			targetComboboxItems={[]}
			hasNoBranches={false}
			startCommitButtonId="start-commit"
			commitMessageInputId="commit-message"
			onAmendCommit={vi.fn()}
			canAmendCommit={false}
			worktreeChanges={worktreeChanges}
		/>
		<PullRequestComments projectId="project-id" review={review} />
	</>
);

const pressModEnter = (target: Element, repeat = false) =>
	act(() => {
		target.dispatchEvent(
			new KeyboardEvent("keydown", {
				bubbles: true,
				cancelable: true,
				ctrlKey: true,
				key: "Enter",
				repeat,
			}),
		);
	});

const enterComment = (composer: HTMLTextAreaElement, value: string) =>
	act(() => {
		// oxlint-disable-next-line typescript/unbound-method -- Invoked with the textarea receiver below.
		const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")?.set;
		if (!setter) throw new Error("Textarea value setter is unavailable");
		Reflect.apply(setter, composer, [value]);
		composer.dispatchEvent(new Event("input", { bubbles: true }));
	});

describe("pull request comment shortcut", () => {
	let container: HTMLDivElement;
	let root: Root;

	beforeEach(() => {
		mutations.commit.mockReset();
		mutations.comment.mockReset();
		container = document.createElement("div");
		document.body.append(container);
		root = createRoot(container);
		act(() => root.render(<Fixture />));
	});

	afterEach(() => {
		act(() => root.unmount());
		container.remove();
		getHotkeyManager().destroy();
	});

	it("posts one focused comment without creating a workspace commit", () => {
		const collapsed = container.querySelector<HTMLButtonElement>('[aria-label="Write a comment"]');
		if (!collapsed) throw new Error("Comment composer did not render");
		act(() => collapsed.focus());

		const composer = container.querySelector<HTMLTextAreaElement>('[aria-label="Write a comment"]');
		if (!composer) throw new Error("Comment input did not expand");
		enterComment(composer, "Ship this");
		pressModEnter(composer, true);
		expect(mutations.comment).not.toHaveBeenCalled();
		expect(mutations.commit).not.toHaveBeenCalled();
		pressModEnter(composer);
		pressModEnter(document.body, true);

		expect(mutations.comment).toHaveBeenCalledTimes(1);
		expect(mutations.commit).not.toHaveBeenCalled();
		const cleared = container.querySelector<HTMLButtonElement>('[aria-label="Write a comment"]');
		if (!cleared) throw new Error("Cleared composer did not collapse");
		act(() => cleared.focus());
		expect(
			container.querySelector<HTMLTextAreaElement>('[aria-label="Write a comment"]')?.value,
		).toBe("");
	});

	it("keeps Mod+Enter with the composer when a footer control is focused", () => {
		const collapsed = container.querySelector<HTMLButtonElement>('[aria-label="Write a comment"]');
		if (!collapsed) throw new Error("Comment composer did not render");
		act(() => collapsed.focus());

		const composer = container.querySelector<HTMLTextAreaElement>('[aria-label="Write a comment"]');
		if (!composer) throw new Error("Comment input did not expand");
		enterComment(composer, "Ship this");
		const commentButton = [...container.querySelectorAll("button")].find((button) =>
			button.textContent.startsWith("Comment"),
		);
		if (!commentButton) throw new Error("Comment button did not render");
		act(() => commentButton.focus());
		pressModEnter(commentButton);

		expect(mutations.comment).toHaveBeenCalledTimes(1);
		expect(mutations.commit).not.toHaveBeenCalled();
	});

	it("commits when Mod+Enter is pressed outside the composer", () => {
		pressModEnter(document.body);

		expect(mutations.commit).toHaveBeenCalledTimes(1);
		expect(mutations.comment).not.toHaveBeenCalled();
	});
});
