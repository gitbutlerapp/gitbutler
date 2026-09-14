import type { TreeChange, UnifiedPatch, WatcherEvent } from "@gitbutler/but-sdk";
import { forwardRef, useImperativeHandle, useLayoutEffect, useRef } from "react";
import { expect, test, vi } from "vitest";
import type * as PierreReact from "@pierre/diffs/react";
import createPanel from "../browser/index.tsx";
import { Page } from "#ui/routes/project/$id/workspace/Page.tsx";
import { uncommittedChangesFileParent, weakFileIdentityKey } from "#ui/addresses.ts";
import { currentParams, setPage } from "#ui/use-cursor.ts";
import { createFakeTransport, createWatcherHandlers } from "./fake-transport.ts";
import {
	fixtureCommit,
	fixtureFileChange,
	fixtureHeadInfo,
	fixtureSegment,
	fixtureWorktreeChanges,
	globalHandlers,
} from "./fixtures.ts";

/**
 * The real `Page` over the streamed diff query, with only the host, the
 * sidebar's input and the CodeView's rendering stood in for. A diff the test
 * withholds keeps its file out of the viewer until released, as a slow diff
 * does; the CodeView stand-in reports what the page asked it to scroll to.
 */

type LineSelection = { id: string; range: unknown };
type CodeViewProps = {
	items: Array<{ id: string }>;
	onScroll: (scrollTop: number, viewer: unknown) => void;
	onSelectedLinesChange: (selection: LineSelection | null) => void;
};

const scrollTo = vi.fn();
/** The props the page last rendered its CodeView with. */
let codeView: CodeViewProps | null = null;

const FIRST_FILE = "file-0.ts";
const SELECTABLE = [FIRST_FILE, "file-64.ts", "file-65.ts"];

vi.mock("#ui/error-reporting.ts", () => ({
	reportError: vi.fn(),
}));

vi.mock("#ui/reviewed-files.ts", () => ({
	reviewedFilesQueryOptions: (projectId: string, contextId: string) => ({
		queryKey: [projectId, "reviewedFiles", contextId],
		queryFn: () => new Map(),
	}),
	useSetFilesReviewed: () => ({ mutate: vi.fn() }),
	usePruneReviewedFiles: () => ({ mutate: vi.fn() }),
}));

vi.mock("#ui/routes/project/$id/workspace/Sidebar.tsx", async () => {
	const { useSaveGUISettings } = await import("#ui/api/mutations.ts");
	const { setActiveList } = await import("#ui/use-cursor.ts");
	return {
		Sidebar: ({
			onActiveFileSelection,
		}: {
			onActiveFileSelection: (selection: string) => void;
		}) => {
			const { mutate: saveGUISettings } = useSaveGUISettings();
			return (
				<>
					{SELECTABLE.map((path) => (
						<button
							key={path}
							type="button"
							data-testid={`select:${path}`}
							onClick={() => {
								setActiveList("uncommitted");
								onActiveFileSelection(path);
							}}
						>
							{path}
						</button>
					))}
					<button
						type="button"
						data-testid="single-file-mode"
						onClick={() => saveGUISettings({ unidiff: false })}
					>
						Single file
					</button>
				</>
			);
		},
	};
});

vi.mock("@pierre/diffs/react", async (importOriginal) => {
	const original = await importOriginal<typeof PierreReact>();
	return {
		...original,
		CodeView: forwardRef((props: CodeViewProps, ref) => {
			const containerRef = useRef<HTMLDivElement>(null);
			// Committed before the page's own layout effects, as the real CodeView's items are.
			useLayoutEffect(() => {
				codeView = props;
			});
			const viewer = {
				getItem: (id: string) => props.items.find((item) => item.id === id),
				getContainerElement: () => containerRef.current,
				getRenderedItems: () => props.items,
				getTopForItem: () => 0,
				scrollTo,
			};
			useImperativeHandle(ref, () => ({
				getInstance: () => viewer,
				getItem: viewer.getItem,
				scrollTo,
			}));
			return <div ref={containerRef} data-testid="code-view" />;
		}),
	};
});

const PROJECT_ID = "streamed-selection";
const settle = { timeout: 15_000 } as const;
const patch: UnifiedPatch = {
	type: "Patch",
	subject: {
		hunks: [
			{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, diff: "@@ -1 +1 @@\n-old\n+new\n" },
		],
		isResultOfBinaryToTextConversion: false,
		linesAdded: 1,
		linesRemoved: 1,
	},
};

/** Long enough for any scroll a publication would have issued to land. */
const nothingElseHappens = () => new Promise((resolve) => setTimeout(resolve, 200));

const itemIdOf = (path: string): string => {
	const id = codeView?.items.find((item) => item.id.startsWith(`file:${path} `))?.id;
	if (id === undefined) throw new Error(`${path} is not in the viewer`);
	return id;
};

/** A viewport scroll that lands on the rendered item at `index`: each item is measured one high. */
const scrollViewport = (index: number): void => {
	if (codeView === null) throw new Error("No CodeView rendered");
	const items = codeView.items;
	codeView.onScroll(index, {
		getRenderedItems: () => items,
		getTopForItem: (id: string) => items.findIndex((item) => item.id === id),
	});
};

const uncommittedCursor = (): string | undefined => currentParams().uncommitted;

/** How many scrolls have targeted the uncommitted file at `path`, in the viewer or not. */
const scrollsTo = (path: string): number => {
	const id = weakFileIdentityKey({ parent: uncommittedChangesFileParent, path });
	return scrollTo.mock.calls.filter(([target]) => (target as { id: string }).id === id).length;
};

const COMMIT_ID = "c".repeat(40);
const commit = fixtureCommit({ id: COMMIT_ID, message: "A committed change" });

/**
 * Mounts the page over `count` uncommitted files. Diffs stream in batches of 64
 * then 128, so files 64 and up are the second batch, published only once every
 * diff in it has arrived; `withheld` names those held back for the test. With
 * `showCommit` the pane opens on a commit's diff instead of the uncommitted files.
 */
const mountPage = async (withheld: Array<string>, { count = 65, showCommit = false } = {}) => {
	codeView = null;
	scrollTo.mockClear();
	const changes = Array.from({ length: count }, (_, index) =>
		fixtureFileChange(`file-${index}.ts`),
	);
	const releases = new Map<string, () => void>();
	const heldPatches = new Map(
		withheld.map((path) => [
			path,
			new Promise<UnifiedPatch>((resolve) => releases.set(path, () => resolve(patch))),
		]),
	);
	// Real diffs outlast a router commit; with the pane opening on a commit, the
	// uncommitted diffs wait so the swapped-in view mounts on the clicked cursor.
	let loadUncommittedDiffs = (): void => {};
	const uncommittedPatches = new Promise<UnifiedPatch>((resolve) => {
		loadUncommittedDiffs = () => resolve(patch);
	});
	const watcher = createWatcherHandlers();
	const fake = createFakeTransport({
		...globalHandlers(PROJECT_ID),
		...watcher.handlers,
		readGUISettings: () => ({ version: 1, unidiff: true }),
		writeGUISettings: () => undefined,
		headInfo: () =>
			fixtureHeadInfo(
				showCommit ? [[fixtureSegment({ branch: "feature", commits: [commit] })]] : [],
			),
		commitDetailsWithLineStats: () => ({
			commit,
			changes: [fixtureFileChange("committed.ts")],
			stats: null,
			conflictEntries: null,
		}),
		branchesList: () => ({ branches: [], addressSpace: { items: [] } }),
		changesInWorktree: () => fixtureWorktreeChanges(changes),
		treeChangeDiffs: ({ change }: { change: TreeChange }) =>
			heldPatches.get(change.path) ??
			(showCommit && change.path !== "committed.ts" ? uncommittedPatches : patch),
		workspaceFetchStatus: () => ({ type: "Idle" }),
		commentsList: () => [],
		forgeInfo: () => null,
		isFullScreen: () => false,
	});
	const app = createPanel({
		transport: fake.transport,
		projectId: PROJECT_ID,
		params: showCommit ? { applied: `change:${commit.changeId}` } : {},
		workspace: Page,
		strict: false,
	});
	const container = document.createElement("div");
	document.body.append(container);
	app.mount(container);

	const button = (testId: string): HTMLButtonElement => {
		const element = container.querySelector<HTMLButtonElement>(`[data-testid="${testId}"]`);
		if (element === null) throw new Error(`No ${testId} button`);
		return element;
	};
	const codeViewRendered = () =>
		vi.waitFor(
			() => expect(container.querySelector('[data-testid="code-view"]')).not.toBeNull(),
			settle,
		);

	await vi.waitFor(() => button(`select:${FIRST_FILE}`), settle);
	await codeViewRendered();
	await nothingElseHappens();
	scrollTo.mockClear();

	return {
		select: (path: string) => button(`select:${path}`).click(),
		loadUncommittedDiffs: () => loadUncommittedDiffs(),
		/** The host announcing the worktree changed: a fresh changes list streams its diffs anew. */
		refreshChanges: () => {
			const eventChannel = watcher.channels.at(0);
			if (eventChannel === undefined) throw new Error("no watcher subscription armed");
			const event: WatcherEvent = {
				name: "worktreeChanges",
				payload: {
					type: "worktreeChanges",
					subject: { changes: fixtureWorktreeChanges(changes), changedPaths: [] },
				},
			};
			fake.push(eventChannel, event);
		},
		singleFileMode: () => button("single-file-mode").click(),
		release: (path: string) => {
			const release = releases.get(path);
			if (release === undefined) throw new Error(`${path} was not withheld`);
			release();
		},
		codeViewRendered,
		codeViewGone: () =>
			vi.waitFor(
				() => expect(container.querySelector('[data-testid="code-view"]')).toBeNull(),
				settle,
			),
		unmount: () => {
			app.unmount();
			container.remove();
		},
	};
};

test("selecting an already-loaded file scrolls once and ignores only the resulting viewport selection", async () => {
	const page = await mountPage(["file-64.ts"]);
	try {
		page.select("file-0.ts");
		expect(scrollTo).toHaveBeenCalledTimes(1);
		expect(scrollTo).toHaveBeenCalledWith({ type: "item", id: itemIdOf("file-0.ts") });

		const before = uncommittedCursor();
		scrollViewport(1);
		expect(uncommittedCursor()).toBe(before);
		scrollViewport(1);
		await vi.waitFor(() => expect(uncommittedCursor()).not.toBe(before), settle);
	} finally {
		page.unmount();
	}
}, 30_000);

test("scrolls to a selected file after its streamed diff arrives", async () => {
	const page = await mountPage(["file-64.ts"]);
	try {
		page.select("file-64.ts");
		await nothingElseHappens();
		expect(scrollTo).not.toHaveBeenCalled();

		page.release("file-64.ts");
		await vi.waitFor(() => expect(scrollTo).toHaveBeenCalledTimes(1), settle);
		expect(scrollTo).toHaveBeenCalledWith({ type: "item", id: itemIdOf("file-64.ts") });
		await nothingElseHappens();
		expect(scrollTo).toHaveBeenCalledTimes(1);

		// Exactly the scroll's own viewport selection is ignored, as for a loaded file.
		const before = uncommittedCursor();
		scrollViewport(0);
		expect(uncommittedCursor()).toBe(before);
		scrollViewport(0);
		await vi.waitFor(() => expect(uncommittedCursor()).not.toBe(before), settle);
	} finally {
		page.unmount();
	}
}, 30_000);

test("the latest selection wins, and reselecting the same file still scrolls once", async () => {
	const page = await mountPage(["file-64.ts", "file-65.ts"], { count: 66 });
	try {
		page.select("file-64.ts");
		page.select("file-64.ts");
		page.select("file-65.ts");
		page.select("file-64.ts");
		page.select("file-65.ts");
		page.release("file-64.ts");
		page.release("file-65.ts");
		await vi.waitFor(() => expect(scrollTo).toHaveBeenCalledTimes(1), settle);
		expect(scrollTo).toHaveBeenCalledWith({ type: "item", id: itemIdOf("file-65.ts") });
		await nothingElseHappens();
		expect(scrollTo).toHaveBeenCalledTimes(1);
	} finally {
		page.unmount();
	}
}, 30_000);

test("a viewport selection before the diff arrives is honoured and cancels the pending scroll", async () => {
	const page = await mountPage(["file-64.ts"]);
	try {
		page.select("file-64.ts");
		const before = uncommittedCursor();
		scrollViewport(0);
		await vi.waitFor(() => expect(uncommittedCursor()).not.toBe(before), settle);

		page.release("file-64.ts");
		await vi.waitFor(() => expect(itemIdOf("file-64.ts")).toBeDefined(), settle);
		await nothingElseHappens();
		expect(scrollTo).not.toHaveBeenCalled();
	} finally {
		page.unmount();
	}
}, 30_000);

test("a line selection before the diff arrives cancels the pending scroll", async () => {
	const page = await mountPage(["file-64.ts"]);
	try {
		page.select("file-64.ts");
		codeView?.onSelectedLinesChange({
			id: itemIdOf("file-0.ts"),
			range: { start: 1, side: "additions", end: 1, endSide: "additions" },
		});

		page.release("file-64.ts");
		await vi.waitFor(() => expect(itemIdOf("file-64.ts")).toBeDefined(), settle);
		await nothingElseHappens();
		expect(scrollTo).not.toHaveBeenCalled();
	} finally {
		page.unmount();
	}
}, 30_000);

test("a diff that never arrives leaves later viewport selections honoured", async () => {
	const page = await mountPage(["file-64.ts"]);
	try {
		page.select("file-64.ts");
		await nothingElseHappens();
		expect(scrollTo).not.toHaveBeenCalled();

		const before = uncommittedCursor();
		scrollViewport(0);
		await vi.waitFor(() => expect(uncommittedCursor()).not.toBe(before), settle);
	} finally {
		page.unmount();
	}
}, 30_000);

test("leaving the all-files view drops the pending scroll", async () => {
	const page = await mountPage(["file-64.ts"]);
	try {
		page.select("file-64.ts");
		page.singleFileMode();
		// The single-file view has no diff to show until the selected file's arrives.
		await page.codeViewGone();

		page.release("file-64.ts");
		await page.codeViewRendered();
		await nothingElseHappens();
		expect(scrollTo).not.toHaveBeenCalled();
	} finally {
		page.unmount();
	}
}, 30_000);

test("returning to the context with the same file cursor still fulfils the pending scroll", async () => {
	const page = await mountPage(["file-64.ts"]);
	try {
		page.select("file-64.ts");
		setPage("branches");
		await page.codeViewGone();
		page.release("file-64.ts");
		await nothingElseHappens();

		setPage("workspace");
		await page.codeViewRendered();
		// Leaving cancelled the stream; a fresh one brings the released diff in.
		page.refreshChanges();
		await vi.waitFor(() => expect(scrollsTo("file-64.ts")).toBe(1), settle);
		await nothingElseHappens();
		expect(scrollsTo("file-64.ts")).toBe(1);
		expect(uncommittedCursor()).toBe("file-64.ts");

		// Exactly the scroll's own viewport selection is ignored; the next one is honoured.
		scrollViewport(0);
		expect(uncommittedCursor()).toBe("file-64.ts");
		scrollViewport(0);
		await vi.waitFor(() => expect(uncommittedCursor()).not.toBe("file-64.ts"), settle);
	} finally {
		page.unmount();
	}
}, 30_000);

test("selecting a withheld uncommitted file while a commit's diff is shown scrolls once on arrival", async () => {
	const page = await mountPage(["file-64.ts"], { showCommit: true });
	try {
		expect(() => itemIdOf("committed.ts")).not.toThrow();
		page.select("file-64.ts");
		await page.codeViewGone();
		await vi.waitFor(() => expect(uncommittedCursor()).toBe("file-64.ts"), settle);
		page.loadUncommittedDiffs();
		await vi.waitFor(() => expect(itemIdOf("file-0.ts")).toBeDefined(), settle);
		// The fresh view's own mount sync lands on the first file; the clicked one waits.
		await nothingElseHappens();
		expect(scrollsTo("file-64.ts")).toBe(0);

		page.release("file-64.ts");
		await vi.waitFor(() => expect(scrollsTo("file-64.ts")).toBe(1), settle);
		expect(scrollTo).toHaveBeenLastCalledWith({ type: "item", id: itemIdOf("file-64.ts") });
		await nothingElseHappens();
		expect(scrollsTo("file-64.ts")).toBe(1);
		expect(uncommittedCursor()).toBe("file-64.ts");
	} finally {
		page.unmount();
	}
}, 30_000);

test("selecting a loaded uncommitted file while a commit's diff is shown keeps the cursor through the replacement's mount scroll", async () => {
	const page = await mountPage(["file-64.ts"], { showCommit: true });
	try {
		page.select("file-0.ts");
		await page.codeViewGone();
		page.loadUncommittedDiffs();
		await vi.waitFor(() => expect(itemIdOf("file-0.ts")).toBeDefined(), settle);
		await vi.waitFor(
			() =>
				expect(scrollTo).toHaveBeenCalledWith(
					expect.objectContaining({ id: itemIdOf("file-0.ts") }),
				),
			settle,
		);

		// The replacement view's own mount scroll must not move the cursor off the clicked file.
		scrollViewport(1);
		expect(uncommittedCursor()).toBe("file-0.ts");
		scrollViewport(1);
		await vi.waitFor(() => expect(uncommittedCursor()).not.toBe("file-0.ts"), settle);
	} finally {
		page.unmount();
	}
}, 30_000);
