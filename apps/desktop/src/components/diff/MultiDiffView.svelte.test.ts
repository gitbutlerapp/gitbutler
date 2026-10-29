import MultiDiffView from "$components/diff/MultiDiffView.svelte";
import { DIFF_SERVICE } from "$lib/hunks/diffService.svelte";
import {
	FILE_SELECTION_MANAGER,
	FileSelectionManager,
} from "$lib/selection/fileSelectionManager.svelte";
import { UI_STATE } from "$lib/state/uiState.svelte";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { tick } from "svelte";
import { SvelteMap } from "svelte/reactivity";
import { get } from "svelte/store";
import { afterAll, afterEach, beforeAll, expect, test, vi } from "vitest";
import type { SelectionId } from "$lib/selection/key";
import type { TreeChange } from "@gitbutler/but-sdk";

vi.mock("$components/diff/FloatingDiffModal.svelte", async () => ({
	default: (
		await import("$components/test/multi-diff-view/FloatingDiffModalInitialIndexOracle.svelte")
	).default,
}));
vi.mock("$components/shared/ChangedFilesContextMenu.svelte", async () => ({
	default: (await import("$components/test/multi-diff-view/ChangedFilesContextMenuHandle.svelte"))
		.default,
}));

class NoopResizeObserver {
	observe() {}
	unobserve() {}
	disconnect() {}
}

class NoopIntersectionObserver {
	observe() {}
	unobserve() {}
	disconnect() {}
}

const clientHeightDescriptor = Object.getOwnPropertyDescriptor(
	HTMLElement.prototype,
	"clientHeight",
);
const offsetHeightDescriptor = Object.getOwnPropertyDescriptor(
	HTMLElement.prototype,
	"offsetHeight",
);
const scrollHeightDescriptor = Object.getOwnPropertyDescriptor(
	HTMLElement.prototype,
	"scrollHeight",
);
const scrollToDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollTo");
const scrollByDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollBy");
const resizeObserverDescriptor = Object.getOwnPropertyDescriptor(globalThis, "ResizeObserver");
const intersectionObserverDescriptor = Object.getOwnPropertyDescriptor(
	globalThis,
	"IntersectionObserver",
);

beforeAll(() => {
	Object.defineProperty(globalThis, "ResizeObserver", {
		configurable: true,
		writable: true,
		value: NoopResizeObserver,
	});
	Object.defineProperty(globalThis, "IntersectionObserver", {
		configurable: true,
		writable: true,
		value: NoopIntersectionObserver,
	});
	Object.defineProperty(HTMLElement.prototype, "clientHeight", {
		configurable: true,
		get() {
			if (this.classList.contains("list-row")) return 175;
			if (this.classList.contains("viewport")) return 100;
			return 0;
		},
	});
	Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
		configurable: true,
		get() {
			return this.clientHeight;
		},
	});
	Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
		configurable: true,
		get() {
			const contents = (this as HTMLElement).querySelector<HTMLElement>(".padded-contents");
			if (!contents) return this.clientHeight;
			const top = Number.parseFloat(contents.style.paddingTop) || 0;
			const bottom = Number.parseFloat(contents.style.paddingBottom) || 0;
			const rows = contents.querySelectorAll(".list-row").length;
			return top + bottom + rows * 175;
		},
	});
	HTMLElement.prototype.scrollTo = function (
		this: HTMLElement,
		options?: ScrollToOptions | number,
		y?: number,
	) {
		if (options === undefined) return;
		this.scrollTop = typeof options === "number" ? (y ?? 0) : (options.top ?? this.scrollTop);
	} as HTMLElement["scrollTo"];
	HTMLElement.prototype.scrollBy = function (
		this: HTMLElement,
		options?: ScrollToOptions | number,
		y?: number,
	) {
		if (options === undefined) return;
		this.scrollTop += typeof options === "number" ? (y ?? 0) : (options.top ?? 0);
	} as HTMLElement["scrollBy"];
});

afterEach(cleanup);

afterAll(() => {
	restoreProperty(HTMLElement.prototype, "clientHeight", clientHeightDescriptor);
	restoreProperty(HTMLElement.prototype, "offsetHeight", offsetHeightDescriptor);
	restoreProperty(HTMLElement.prototype, "scrollHeight", scrollHeightDescriptor);
	restoreProperty(HTMLElement.prototype, "scrollTo", scrollToDescriptor);
	restoreProperty(HTMLElement.prototype, "scrollBy", scrollByDescriptor);
	restoreProperty(globalThis, "ResizeObserver", resizeObserverDescriptor);
	restoreProperty(globalThis, "IntersectionObserver", intersectionObserverDescriptor);
});

const selectionId: SelectionId = { type: "worktree", stackId: undefined };

test("insert-before keeps B as preview and real VirtualList initial viewport", async () => {
	const initial = changes();
	const selectedIndex = 8;
	initial[selectedIndex] = change("b.ts");
	const mounted = mount(initial, selectedIndex);
	const updated = [change("a.ts"), ...initial];

	await mounted.rerender({ ...mounted.props, changes: updated });
	const observed = await observeTransition(mounted, updated.length, selectedIndex + 1);

	expect(observed).toMatchObject({
		selectionBeforeMount: [mounted.selected.path],
		selectionAfterMount: [mounted.selected.path],
		previewPath: mounted.selected.path,
		initialViewportIndex: selectedIndex + 1,
		mountedTarget: true,
		renderedEveryRow: false,
		scrollInTargetBand: true,
	});
});

test("remove-before keeps B as preview and real VirtualList initial viewport", async () => {
	const initial = [change("a.ts"), ...changes()];
	const selectedIndex = 9;
	initial[selectedIndex] = change("b.ts");
	const mounted = mount(initial, selectedIndex);
	const updated = initial.slice(1);

	await mounted.rerender({ ...mounted.props, changes: updated });
	const observed = await observeTransition(mounted, updated.length, selectedIndex - 1);

	expect(observed).toMatchObject({
		selectionBeforeMount: [mounted.selected.path],
		selectionAfterMount: [mounted.selected.path],
		previewPath: mounted.selected.path,
		initialViewportIndex: selectedIndex - 1,
		mountedTarget: true,
		renderedEveryRow: false,
		scrollInTargetBand: true,
	});
});

test.each(["insert", "remove"] as const)(
	"%s-before keeps B current while the real VirtualList stays mounted",
	async (direction) => {
		const initial = direction === "insert" ? changes() : [change("a.ts"), ...changes()];
		const selectedIndex = direction === "insert" ? 8 : 9;
		initial[selectedIndex] = change("b.ts");
		const mounted = mount(initial, selectedIndex, vi.fn(), true);
		await waitFor(() => expect(viewportObservation().effectiveIndex).toBe(selectedIndex));
		const viewport = virtualListViewport();
		const initialScrollTop = viewport.scrollTop;
		mounted.visibleSelections.length = 0;

		const updated = direction === "insert" ? [change("a.ts"), ...initial] : initial.slice(1);
		const targetIndex = direction === "insert" ? selectedIndex + 1 : selectedIndex - 1;
		await mounted.rerender({ ...mounted.props, changes: updated });
		await waitFor(() => {
			expect(virtualListViewport()).toBe(viewport);
			expect(selectedPaths(mounted.selection)).toEqual(["b.ts"]);
			expect(dominantViewportRow()).toMatchObject({ index: targetIndex, path: "b.ts" });
		});
		expect(
			direction === "insert"
				? viewport.scrollTop > initialScrollTop
				: viewport.scrollTop < initialScrollTop,
		).toBe(true);
		const targetRow = document.querySelector<HTMLElement>(
			`.list-row[data-index="${targetIndex}"]`,
		)!;
		expect(targetRow.querySelector(".drawer.highlighted")).not.toBeNull();
		expect(document.querySelectorAll(".list-row").length).toBeLessThan(updated.length);
		expect(mounted.visibleSelections).not.toHaveLength(0);
		expect(
			mounted.visibleSelections.every((paths) => paths.length === 1 && paths[0] === "b.ts"),
		).toBe(true);

		viewport.scrollTop += 10;
		await dispatchScroll(viewport);
		expect(selectedPaths(mounted.selection)).toEqual(["b.ts"]);
		expect(targetRow.querySelector(".drawer.highlighted")).not.toBeNull();

		const buttons = document.querySelectorAll<HTMLButtonElement>(".floating-actions button");
		expect(buttons).toHaveLength(2);
		await fireEvent.click(buttons[0]!);
		expect(screen.getByTestId("floating-diff")).toHaveAttribute(
			"data-initial-index",
			String(targetIndex),
		);
	},
);

test("intentional navigation follows lastAdded including a reverse range", async () => {
	const fileChanges = [change("a.ts"), change("b.ts"), change("c.ts")];
	const mounted = mount(fileChanges, 0);

	mounted.selection.set("b.ts", selectionId, 1);
	mounted.component.jumpToIndex(1);
	await tick();
	expect(previewPath()).toBe("b.ts");

	mounted.selection.set("c.ts", selectionId, 2);
	mounted.selection.addMany(["a.ts", "b.ts", "c.ts"], selectionId, {
		path: "a.ts",
		index: 0,
	});
	mounted.component.jumpToIndex(0);
	await tick();

	expect(selectedPaths(mounted.selection)).toEqual(["c.ts", "a.ts", "b.ts"]);
	expect(get(mounted.selection.getById(selectionId).lastAdded)?.index).toBe(0);
	expect(previewPath()).toBe("a.ts");
});

test("standalone jump changes the single-file preview positionally", async () => {
	const mounted = mount([change("a.ts"), change("b.ts"), change("c.ts")], 0);

	mounted.component.jumpToIndex(2);
	await tick();

	expect(previewPath()).toBe("c.ts");
	expect(selectedPaths(mounted.selection)).toEqual(["a.ts"]);
	expect(get(mounted.selection.getById(selectionId).lastAdded)?.index).toBe(0);
});

test("a later canonical selection supersedes a standalone jump", async () => {
	const fileChanges = [change("a.ts"), change("b.ts"), change("c.ts")];
	const mounted = mount(fileChanges, 0);

	mounted.component.jumpToIndex(2);
	await tick();
	expect(previewPath()).toBe("c.ts");
	expect(selectedPaths(mounted.selection)).toEqual(["a.ts"]);

	mounted.selection.set("b.ts", { ...selectionId }, 1);
	await tick();

	expect(previewPath()).toBe("b.ts");
	expect(selectedPaths(mounted.selection)).toEqual(["b.ts"]);
});

test("standalone jump ownership follows the stable selection key", async () => {
	const fileChanges = [change("a.ts"), change("b.ts"), change("c.ts")];
	const equivalentCollection: SelectionId = { type: "worktree", stackId: undefined };
	const collectionB: SelectionId = { type: "worktree", stackId: "other" };
	const mounted = mount(fileChanges, 0);

	mounted.component.jumpToIndex(2);
	await tick();
	expect(previewPath()).toBe("c.ts");
	await mounted.rerender({ ...mounted.props, selectionId: equivalentCollection });
	expect(previewPath()).toBe("c.ts");
	expect(selectedPaths(mounted.selection, equivalentCollection)).toEqual(["a.ts"]);

	mounted.selection.set("b.ts", collectionB, 1);
	await mounted.rerender({ ...mounted.props, selectionId: collectionB });
	expect(previewPath()).toBe("b.ts");
	expect(mounted.selection.values(collectionB).map(({ path }) => path)).toEqual(["b.ts"]);

	await mounted.rerender({ ...mounted.props, selectionId: { ...selectionId } });
	expect(previewPath()).toBe("a.ts");
	expect(selectedPaths(mounted.selection)).toEqual(["a.ts"]);
});

test("a new changes array resolves a pre-prop jump by the selected path", async () => {
	const initial = [change("old-a.ts"), change("shared.ts"), change("old-c.ts")];
	const mounted = mount(initial, 0);

	mounted.selection.set("target.ts", selectionId, 1);
	mounted.component.jumpToIndex(1);
	await tick();
	expect(previewPath()).toBe("shared.ts");
	expect(selectedPaths(mounted.selection)).toEqual(["target.ts"]);
	expect(get(mounted.selection.getById(selectionId).lastAdded)?.index).toBe(1);

	const next = [change("shared.ts"), change("new-a.ts"), change("target.ts")];
	await mounted.rerender({ ...mounted.props, changes: next });

	expect(previewPath()).toBe("target.ts");
	expect(selectedPaths(mounted.selection)).toEqual(["target.ts"]);
	expect(get(mounted.selection.getById(selectionId).lastAdded)?.index).toBe(1);

	mounted.selection.set("old-c.ts", selectionId, 0);
	await mounted.rerender({ ...mounted.props, changes: initial });

	expect(previewPath()).toBe("old-c.ts");
	expect(selectedPaths(mounted.selection)).toEqual(["old-c.ts"]);
});

test("single-mode pop-out uses the resolved selected path index", async () => {
	const initial = changes();
	initial[8] = change("b.ts");
	const mounted = mount(initial, 8, vi.fn());

	await mounted.rerender({ ...mounted.props, changes: [change("a.ts"), ...initial] });
	const buttons = document.querySelectorAll<HTMLButtonElement>(
		".single-diff-view .drawer-header__actions button",
	);
	expect(buttons).toHaveLength(3);
	await fireEvent.click(buttons[1]!);

	expect(screen.getByTestId("floating-diff")).toHaveAttribute("data-initial-index", "9");
});

test("all-in-one visible range stays positional and does not replace a multi-selection", async () => {
	const fileChanges = changes();
	fileChanges[8] = change("b.ts");
	const mounted = mount(fileChanges, 8, vi.fn());
	mounted.selection.add(fileChanges[9]!.path, selectionId, 9);
	mounted.settings.set("allInOneDiff", true);
	await tick();
	await waitFor(() => expect(document.querySelectorAll(".list-row").length).toBeGreaterThan(0));

	const viewport = virtualListViewport();
	viewport.scrollTop += 10;
	await dispatchScroll(viewport);
	expect(selectedPaths(mounted.selection)).toEqual(["b.ts", fileChanges[9]!.path]);

	viewport.scrollTop = 18 * 173;
	await dispatchScroll(viewport);
	await waitFor(() => {
		const observation = viewportObservation();
		expect(observation.effectiveIndex).toBeGreaterThanOrEqual(17);
	});
	const observation = viewportObservation();
	expect(selectedPaths(mounted.selection)).toEqual(["b.ts", fileChanges[9]!.path]);

	const buttons = document.querySelectorAll<HTMLButtonElement>(".floating-actions button");
	expect(buttons).toHaveLength(2);
	await fireEvent.click(buttons[0]!);
	expect(screen.getByTestId("floating-diff")).toHaveAttribute(
		"data-initial-index",
		String(observation.effectiveIndex),
	);
});

test("all-in-one selection lock pins a single selection and releases after it leaves view", async () => {
	const fileChanges = changes();
	fileChanges[8] = change("b.ts");
	const mounted = mount(fileChanges, 8);
	mounted.settings.set("allInOneDiff", true);
	await tick();
	await waitFor(() => expect(document.querySelectorAll(".list-row").length).toBeGreaterThan(0));
	expect(selectedPaths(mounted.selection)).toEqual(["b.ts"]);

	const viewport = virtualListViewport();
	viewport.scrollTop += 10;
	await dispatchScroll(viewport);
	expect(selectedPaths(mounted.selection)).toEqual(["b.ts"]);

	viewport.scrollTop = 18 * 173;
	await dispatchScroll(viewport);
	await waitFor(() => expect(selectedPaths(mounted.selection)).not.toEqual(["b.ts"]));
	const activeIndex = viewportObservation().effectiveIndex!;
	expect(selectedPaths(mounted.selection)).toEqual([fileChanges[activeIndex]!.path]);
});

test("missing selected path clamps the numeric fallback and empty changes show the placeholder", async () => {
	const fileChanges = [change("a.ts"), change("b.ts"), change("c.ts")];
	const mounted = mount(fileChanges, 2);

	await mounted.rerender({ ...mounted.props, changes: fileChanges.slice(0, 2) });
	expect(previewPath()).toBe("b.ts");

	await mounted.rerender({ ...mounted.props, changes: [] });
	expect(screen.getByText("Select a file to preview")).toBeInTheDocument();
});

test("single and all-in-one close controls keep calling onclose", async () => {
	const singleClose = vi.fn();
	const single = mount([change("a.ts")], 0, singleClose);
	const singleButtons = document.querySelectorAll<HTMLButtonElement>(
		".single-diff-view .drawer-header__actions button",
	);
	expect(singleButtons).toHaveLength(3);
	await fireEvent.click(singleButtons[2]!);
	expect(singleClose).toHaveBeenCalledOnce();
	single.unmount();

	const allInOneClose = vi.fn();
	const allInOne = mount(changes(), 0, allInOneClose);
	allInOne.settings.set("allInOneDiff", true);
	await tick();
	const allInOneButtons = document.querySelectorAll<HTMLButtonElement>(".floating-actions button");
	expect(allInOneButtons).toHaveLength(2);
	await fireEvent.click(allInOneButtons[1]!);
	expect(allInOneClose).toHaveBeenCalledOnce();
});

function mount(
	fileChanges: TreeChange[],
	selectedIndex: number,
	onclose?: () => void,
	allInOneDiff = false,
) {
	const selection = new FileSelectionManager(null!, null!, null!, null!, null!);
	const selected = fileChanges[selectedIndex]!;
	selection.set(selected.path, selectionId, selectedIndex);
	const visibleSelections: string[][] = [];
	const settings = new SvelteMap([
		["allInOneDiff", allInOneDiff],
		["highlightDiffs", true],
	]);
	const props = {
		projectId: "project",
		changes: fileChanges,
		selectable: true,
		selectionId,
		startIndex: selectedIndex,
		onclose,
		onVisibleChange: (range: { start: number; end: number } | undefined) => {
			if (range) visibleSelections.push(selectedPaths(selection));
		},
	};
	const context = new Map<unknown, unknown>([
		[DIFF_SERVICE._key, { getDiff: () => ({ response: undefined, result: undefined }) }],
		[FILE_SELECTION_MANAGER._key, selection],
		[
			UI_STATE._key,
			{
				global: {
					scrollbarVisibilityState: { current: "scroll" },
					allInOneDiff: {
						get current() {
							return settings.get("allInOneDiff") ?? false;
						},
					},
					highlightDiffs: {
						get current() {
							return settings.get("highlightDiffs") ?? false;
						},
					},
				},
			},
		],
	]);
	const view = render(MultiDiffView, { props, context });
	return { ...view, props, selected, selection, settings, visibleSelections };
}

async function observeTransition(
	mounted: ReturnType<typeof mount>,
	totalItems: number,
	targetIndex: number,
) {
	const currentPreviewPath = previewPath();
	const selectionBeforeMount = selectedPaths(mounted.selection);
	mounted.settings.set("allInOneDiff", true);
	await tick();
	await waitFor(() => expect(document.querySelectorAll(".list-row").length).toBeGreaterThan(0));
	const rows = [...document.querySelectorAll<HTMLElement>(".list-row")];
	const mountedIndexes = rows.map((row) => Number(row.dataset.index));
	const viewport = viewportObservation();
	return {
		selectionBeforeMount,
		selectionAfterMount: selectedPaths(mounted.selection),
		previewPath: currentPreviewPath,
		initialViewportIndex: viewport.effectiveIndex,
		mountedIndexes,
		mountedTarget: mountedIndexes.includes(targetIndex),
		renderedEveryRow: rows.length === totalItems,
		scrollTop: viewport.scrollTop,
		scrollInTargetBand: viewport.effectiveIndex === targetIndex,
	};
}

function viewportObservation() {
	const viewport = virtualListViewport();
	const contents = viewport.querySelector<HTMLElement>(".padded-contents")!;
	const rows = [...contents.querySelectorAll<HTMLElement>(":scope > .list-row")];
	let top = Number.parseFloat(contents.style.paddingTop) || 0;
	let effectiveIndex: number | undefined;
	for (const row of rows) {
		const bottom = top + row.clientHeight;
		if (viewport.scrollTop >= top && viewport.scrollTop < bottom) {
			effectiveIndex = Number(row.dataset.index);
			break;
		}
		top = bottom;
	}
	return { effectiveIndex, scrollTop: viewport.scrollTop };
}

function dominantViewportRow() {
	const viewport = virtualListViewport();
	const contents = viewport.querySelector<HTMLElement>(".padded-contents")!;
	const viewportTop = viewport.scrollTop;
	const viewportBottom = viewportTop + viewport.clientHeight;
	let top = Number.parseFloat(contents.style.paddingTop) || 0;
	let dominant: { index: number; path: string | undefined; visibleHeight: number } | undefined;

	for (const row of contents.querySelectorAll<HTMLElement>(":scope > .list-row")) {
		const bottom = top + row.clientHeight;
		const visibleHeight = Math.max(
			0,
			Math.min(bottom, viewportBottom) - Math.max(top, viewportTop),
		);
		if (!dominant || visibleHeight > dominant.visibleHeight) {
			dominant = {
				index: Number(row.dataset.index),
				path: row.querySelector<HTMLElement>(".file-name__name")?.textContent?.trim(),
				visibleHeight,
			};
		}
		top = bottom;
	}
	return dominant;
}

function virtualListViewport() {
	return document
		.querySelector<HTMLElement>(".padded-contents")!
		.closest<HTMLElement>(".viewport")!;
}

function previewPath() {
	return document
		.querySelector<HTMLElement>(".single-diff-view .file-name__name")
		?.textContent?.trim();
}

async function dispatchScroll(viewport: HTMLElement) {
	viewport.dispatchEvent(new Event("scroll"));
	await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
	await tick();
}

function selectedPaths(selection: FileSelectionManager, id = selectionId) {
	return selection.values(id).map(({ path }) => path);
}

function changes() {
	return Array.from({ length: 24 }, (_, index) => change(`f${String(index).padStart(2, "0")}.ts`));
}

function change(path: string): TreeChange {
	return {
		path,
		pathBytes: [],
		status: {
			type: "Addition",
			subject: { state: { id: path, kind: "Blob" }, isUntracked: false },
		},
	};
}

function restoreProperty(
	target: object,
	property: PropertyKey,
	descriptor: PropertyDescriptor | undefined,
) {
	if (descriptor) Object.defineProperty(target, property, descriptor);
	else Reflect.deleteProperty(target, property);
}
