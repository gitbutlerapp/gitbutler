/** @vitest-environment jsdom */

import type { FileParent } from "#ui/addresses.ts";
import type { NativeMenuItem } from "#ui/native-menu.ts";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, useEffect, type FC } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

declare global {
	var IS_REACT_ACT_ENVIRONMENT: boolean | undefined;
}

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const hookState = vi.hoisted(() => ({
	/** Addresses the checked set holds, and whether it is wholly files under this parent. */
	checkedAddresses: [] as Array<{ _tag: "File"; parent: unknown; path: string }>,
	canCheckFiles: true,
	discard: vi.fn(),
	uncommit: vi.fn(),
	resolveWorktreeConflicts: vi.fn(),
	cut: vi.fn(),
	setFilesReviewed: vi.fn(),
}));

vi.mock("#ui/api/mutations.ts", () => ({
	useDiscardFileChanges: () => ({ canDiscard: true, discard: hookState.discard }),
	useCommitUncommitChanges: () => ({ isPending: false, mutate: hookState.uncommit }),
	useResolveWorktreeConflicts: () => ({
		isPending: false,
		mutate: hookState.resolveWorktreeConflicts,
	}),
}));
vi.mock("#ui/api/queries.ts", () => ({
	changesInWorktreeQueryOptions: () => ({ queryKey: ["changes"] }),
	// The real diffs are the backend's; a review only needs one entry per change,
	// which the stubbed `prepareDiffFiles` below turns into a version.
	treeChangesDiffsQueryOptions: ({ changes }: { changes: Array<{ path: string }> }) => ({
		queryKey: ["treeChangeDiffs", changes],
		queryFn: () => changes.map(() => null),
	}),
}));
vi.mock("#ui/reviewed-files.ts", () => ({
	useSetFilesReviewed: () => ({ mutate: hookState.setFilesReviewed }),
}));
vi.mock("./diff-view.ts", () => ({
	prepareDiffFiles: ({ changes }: { changes: Array<{ path: string }> }) =>
		changes.map((change) => ({ change, version: 1 })),
}));
vi.mock("#ui/operations/diff-specs.ts", () => ({ resolveDiffSpecs: async () => [] }));
vi.mock("#ui/use-cursor.ts", () => ({
	startAbsorb: vi.fn(),
	startKeyboardTransfer: hookState.cut,
}));
vi.mock("#ui/focus-scopes.ts", () => ({ focusScope: vi.fn() }));
vi.mock("#ui/hotkeys.ts", () => ({
	changesFileHotkeys: {
		absorb: { hotkey: "A" },
		discard: { hotkey: "Mod+Backspace" },
		toggleFoldDirectory: { hotkey: "Z" },
		uncommit: { hotkey: "Mod+Alt+Backspace" },
	},
	selectionOperationHotkeys: { cut: { hotkey: "Mod+X" } },
	toElectronAccelerator: () => "",
}));
vi.mock("#ui/projects/state.ts", () => ({
	projectSlice: {
		selectors: {
			selectCanCheckFiles: () => hookState.canCheckFiles,
			selectCheckedAddressCount: () => hookState.checkedAddresses.length,
			selectCheckedAddresses: () => hookState.checkedAddresses,
		},
	},
}));
vi.mock("#ui/store.ts", () => ({
	useAppStore: () => ({ getState: () => ({}) }),
	useAppSelector: (selector: (state: object) => unknown) => selector({}),
}));
// The path acts are a file row's, unchanged; what a directory does with them is not
// what these tests are about.
vi.mock("./usePathMenuItems.ts", () => ({ usePathMenuItems: () => [] }));

import { changeFileRowItem, conflictFileRowItem, type FileRowItem } from "./file-row.ts";
import { useDirectoryMenuItems } from "./useDirectoryMenuItems.ts";

const projectId = "project-id";
const uncommitted: FileParent = { _tag: "UncommittedChanges" };
const commit: FileParent = { _tag: "Commit", commitId: "commit-id", changeId: "change-id" };
const branch: FileParent = { _tag: "Branch", branchRef: [1] };
const linkedWorktree: FileParent = { _tag: "UncommittedChanges", worktree: "feature" };

const change = (path: string): FileRowItem =>
	changeFileRowItem({
		change: { path, pathBytes: [], status: { type: "Modification" } } as never,
		dependencyCommitIds: [],
		path,
	});

let items: Array<NativeMenuItem> = [];

const render = (options: {
	fileParent: FileParent;
	items: Array<FileRowItem>;
	checkedState?: "checked" | "indeterminate" | "unchecked";
	isReviewed?: boolean;
}): void => {
	const Probe: FC = () => {
		const menuItems = useDirectoryMenuItems({
			projectId,
			fileParent: options.fileParent,
			path: "src/ui",
			items: options.items,
			checkedState: options.checkedState ?? "unchecked",
			isReviewed: options.isReviewed ?? false,
			isCollapsed: false,
			onToggleCollapsed: () => {},
		});
		// Published from an effect rather than during render, which would be a side effect.
		useEffect(() => {
			items = menuItems;
		});
		return null;
	};

	act(() => {
		root.render(
			<QueryClientProvider client={queryClient}>
				<Probe />
			</QueryClientProvider>,
		);
	});
};

const labels = (): Array<string> =>
	items.filter((item) => item._tag === "Item").map((item) => item.label);

const select = (label: string): void => {
	const item = items.find((item) => item._tag === "Item" && item.label === label);
	if (item === undefined || item._tag !== "Item") throw new Error(`no menu item "${label}"`);
	act(() => void item.onSelect?.());
};

/** The same, for an act that has to fetch the diffs its subject is named by. */
const selectAndSettle = async (label: string): Promise<void> => {
	select(label);
	await act(async () => {
		await queryClient.getQueryCache().getAll().at(-1)?.promise;
	});
};

let container: HTMLDivElement;
let queryClient: QueryClient;
let root: Root;

describe("useDirectoryMenuItems", () => {
	beforeEach(() => {
		hookState.checkedAddresses = [];
		hookState.canCheckFiles = true;
		hookState.discard.mockClear();
		hookState.uncommit.mockClear();
		hookState.resolveWorktreeConflicts.mockClear();
		hookState.cut.mockClear();
		hookState.setFilesReviewed.mockClear();
		container = document.createElement("div");
		document.body.append(container);
		queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
		root = createRoot(container);
	});

	afterEach(() => {
		act(() => root.unmount());
		container.remove();
		vi.unstubAllGlobals();
	});

	it("counts the files below it in every label", () => {
		render({ fileParent: uncommitted, items: [change("src/ui/a.ts"), change("src/ui/b.ts")] });

		expect(labels()).toEqual([
			"Cut 2 Files",
			"Absorb 2 Files",
			"Discard Changes in 2 Files",
			"Mark 2 Files as Reviewed",
			"Collapse",
		]);
	});

	it("says what a file row says when it holds one file", () => {
		render({ fileParent: uncommitted, items: [change("src/ui/a.ts")] });

		expect(labels()).toEqual([
			"Cut File",
			"Absorb",
			"Discard Changes",
			"Mark as Reviewed",
			"Collapse",
		]);
	});

	it("offers uncommitting rather than absorbing under a commit", () => {
		render({ fileParent: commit, items: [change("src/ui/a.ts"), change("src/ui/b.ts")] });

		expect(labels()).toContain("Uncommit 2 Files");
		expect(labels()).not.toContain("Absorb 2 Files");
	});

	it("offers reviewing but no other acts on branch files", () => {
		render({ fileParent: branch, items: [change("src/ui/a.ts")] });

		expect(labels()).toEqual(["Mark as Reviewed", "Collapse"]);
	});

	it("offers cutting but no absorbing or discarding in a linked worktree", () => {
		render({ fileParent: linkedWorktree, items: [change("src/ui/a.ts")] });

		expect(labels()).toEqual(["Cut File", "Mark as Reviewed", "Collapse"]);
	});

	it("offers resolving only what is conflicted below it", () => {
		render({
			fileParent: uncommitted,
			items: [
				change("src/ui/a.ts"),
				conflictFileRowItem({ path: "src/ui/b.ts" }),
				conflictFileRowItem({ path: "src/ui/c.ts" }),
			],
		});

		expect(labels()).toContain("Mark 2 Files as Resolved");
		// The one change below is what the acts are addressed to; the conflicts are not.
		expect(labels()).toContain("Discard Changes");

		select("Mark 2 Files as Resolved");
		expect(hookState.resolveWorktreeConflicts).toHaveBeenCalledWith({
			projectId,
			paths: ["src/ui/b.ts", "src/ui/c.ts"],
		});
	});

	it("acts on its own files while its box is not fully checked", () => {
		hookState.checkedAddresses = [{ _tag: "File", parent: uncommitted, path: "elsewhere.ts" }];
		render({
			fileParent: uncommitted,
			items: [change("src/ui/a.ts")],
			checkedState: "indeterminate",
		});

		expect(labels()).toContain("Discard Changes");

		select("Discard Changes");
		expect(hookState.discard).toHaveBeenCalledWith([
			{ _tag: "File", parent: uncommitted, path: "src/ui/a.ts" },
		]);
	});

	it("gives way to the checked set once its box reads checked", () => {
		hookState.checkedAddresses = [
			{ _tag: "File", parent: uncommitted, path: "src/ui/a.ts" },
			{ _tag: "File", parent: uncommitted, path: "elsewhere.ts" },
		];
		render({ fileParent: uncommitted, items: [change("src/ui/a.ts")], checkedState: "checked" });

		expect(labels()).toContain("Discard Changes in 2 Files");

		select("Discard Changes in 2 Files");
		expect(hookState.discard).toHaveBeenCalledWith(hookState.checkedAddresses);
	});

	it("marks every change below it reviewed, leaving its conflicts alone", async () => {
		render({
			fileParent: uncommitted,
			items: [
				change("src/ui/a.ts"),
				change("src/ui/b.ts"),
				conflictFileRowItem({ path: "src/ui/c.ts" }),
			],
		});

		await selectAndSettle("Mark 2 Files as Reviewed");
		expect(hookState.setFilesReviewed).toHaveBeenCalledWith(
			expect.objectContaining({
				projectId,
				files: [
					{ path: "src/ui/a.ts", version: 1 },
					{ path: "src/ui/b.ts", version: 1 },
				],
				reviewed: true,
			}),
		);
	});

	it("reads the other way round once every change below it is reviewed", async () => {
		render({
			fileParent: uncommitted,
			items: [change("src/ui/a.ts")],
			isReviewed: true,
		});

		expect(labels()).toContain("Mark as Unreviewed");

		await selectAndSettle("Mark as Unreviewed");
		expect(hookState.setFilesReviewed).toHaveBeenCalledWith(
			expect.objectContaining({ files: [{ path: "src/ui/a.ts", version: 1 }], reviewed: false }),
		);
	});

	it("reviews its own files however the checked set stands", async () => {
		hookState.checkedAddresses = [
			{ _tag: "File", parent: uncommitted, path: "src/ui/a.ts" },
			{ _tag: "File", parent: uncommitted, path: "elsewhere.ts" },
		];
		render({ fileParent: uncommitted, items: [change("src/ui/a.ts")], checkedState: "checked" });

		// The acts above give way to the checked set; how far a reader has got does not.
		expect(labels()).toContain("Discard Changes in 2 Files");
		expect(labels()).toContain("Mark as Reviewed");

		await selectAndSettle("Mark as Reviewed");
		expect(hookState.setFilesReviewed).toHaveBeenCalledWith(
			expect.objectContaining({ files: [{ path: "src/ui/a.ts", version: 1 }] }),
		);
	});

	it("keeps to its own files when the checked set is not wholly ours", () => {
		hookState.canCheckFiles = false;
		hookState.checkedAddresses = [{ _tag: "File", parent: uncommitted, path: "src/ui/a.ts" }];
		render({ fileParent: uncommitted, items: [change("src/ui/a.ts")], checkedState: "checked" });

		expect(labels()).toContain("Discard Changes");
	});
});
