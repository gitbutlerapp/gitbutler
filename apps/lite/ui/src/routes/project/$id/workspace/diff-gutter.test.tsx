/** @vitest-environment jsdom */

import { hunkAddress, type HunkAddress } from "#ui/addresses.ts";
import { store } from "#ui/store.ts";
import { processFile, type CodeViewDiffItem } from "@pierre/diffs";
import { act, createRef, forwardRef, type RefObject, useImperativeHandle } from "react";
import { Provider } from "react-redux";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDiffGutterCheckboxes } from "./diff-gutter.ts";

declare global {
	var IS_REACT_ACT_ENVIRONMENT: boolean | undefined;
}

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
// jsdom ships no CSS object; the gutter only ever escapes line indices with it.
Reflect.set(globalThis, "CSS", { escape: (value: string) => value });

const ITEM = { type: "diff", id: "file.ts", version: 1 } as unknown as CodeViewDiffItem<unknown>;

const HUNK: HunkAddress = {
	parent: { parent: { _tag: "UncommittedChanges" }, path: "file.ts" },
	isResultOfBinaryToTextConversion: false,
	hunkHeader: { oldStart: 1, oldLines: 1, newStart: 1, newLines: 1 },
	lineGroups: [{ side: "additions", start: 2, lines: 1 }],
} as unknown as HunkAddress;

type GutterHandle = ReturnType<typeof useDiffGutterCheckboxes<unknown>>;

/** One line per number, so a drag can be asked which lines it painted. */
const lineAddress = ({ lineNumber }: { lineNumber: number }) =>
	hunkAddress({ ...HUNK, lineGroups: [{ side: "additions", start: lineNumber, lines: 1 }] });

const onCheckLine = vi.fn<(address: HunkAddress, shiftKey: boolean) => void>();

const Probe = forwardRef<GutterHandle>((_props, ref) => {
	const result = useDiffGutterCheckboxes<unknown>(
		vi.fn(),
		lineAddress,
		() => hunkAddress(HUNK),
		"project",
		onCheckLine,
		vi.fn(),
	);
	useImperativeHandle(ref, () => result, [result]);
	return result.portals;
});

/** A number cell and the code beside it, the two halves the pointer crosses between. */
const createHost = (): { host: HTMLElement; cell: HTMLElement; code: HTMLElement } => {
	const host = document.createElement("div");
	const shadowRoot = host.attachShadow({ mode: "open" });
	const cell = document.createElement("span");
	cell.setAttribute("data-column-number", "2");
	cell.setAttribute("data-line-type", "change-addition");
	cell.setAttribute("data-line-index", "0");
	const code = document.createElement("span");
	code.setAttribute("data-line", "2");
	code.setAttribute("data-line-type", "change-addition");
	code.setAttribute("data-line-index", "0");
	shadowRoot.append(cell, code);
	return { host, cell, code };
};

const pointerOver = (element: HTMLElement) =>
	act(() => {
		element.dispatchEvent(new Event("pointerover", { bubbles: true, composed: true }));
	});

type ColumnLine = {
	number: number;
	type?: "change-addition" | "change-deletion" | "context" | "context-expanded";
};

/**
 * A column of number cells laid out the way Pierre renders one: a code element holding a gutter
 * whose children are the cells. jsdom has no layout, so the cell under a point is whichever one
 * the test last said it was.
 */
const createColumn = (
	lines: Array<ColumnLine>,
): {
	host: HTMLElement;
	cells: Array<HTMLElement>;
	pointAt: (cell: HTMLElement) => void;
	/** Rebuilds every cell from scratch, as a re-render of the diff does. */
	rerender: () => Array<HTMLElement>;
} => {
	const host = document.createElement("div");
	const shadowRoot = host.attachShadow({ mode: "open" });
	const column = document.createElement("code");
	column.setAttribute("data-code", "");
	column.setAttribute("data-unified", "");
	const gutter = document.createElement("div");
	gutter.setAttribute("data-gutter", "");
	column.append(gutter);
	shadowRoot.append(column);
	const build = (): Array<HTMLElement> =>
		lines.map(({ number, type = "change-addition" }, index) => {
			const cell = document.createElement("div");
			cell.setAttribute("data-column-number", `${number}`);
			cell.setAttribute("data-line-type", type);
			cell.setAttribute("data-line-index", `${index},${index}`);
			const numberContent = document.createElement("span");
			numberContent.setAttribute("data-line-number-content", "");
			numberContent.textContent = String(number);
			cell.append(numberContent);
			return cell;
		});
	const cells = build();
	gutter.append(...cells);
	let under: HTMLElement | null = null;
	Reflect.set(shadowRoot, "elementFromPoint", () => under);
	return {
		host,
		cells,
		pointAt: (cell) => void (under = cell),
		rerender: () => {
			const next = build();
			gutter.replaceChildren(...next);
			return next;
		},
	};
};

const pointer = (type: string, init: PointerEventInit = {}): PointerEvent =>
	new PointerEvent(type, { bubbles: true, composed: true, button: 0, ...init });

describe("useDiffGutterCheckboxes", () => {
	let container: HTMLDivElement;
	let root: Root;
	let handleRef: RefObject<GutterHandle | null>;

	const handle = (): GutterHandle => {
		if (!handleRef.current) throw new Error("Probe did not expose the gutter handle");
		return handleRef.current;
	};

	beforeEach(() => {
		onCheckLine.mockReset();
		container = document.createElement("div");
		document.body.append(container);
		root = createRoot(container);
		handleRef = createRef<GutterHandle>();
		act(() =>
			root.render(
				<Provider store={store}>
					<Probe ref={handleRef} />
				</Provider>,
			),
		);
	});

	afterEach(() => {
		act(() => root.unmount());
		container.remove();
	});

	it("keeps old and new numbers distinct across a replacement and expanded context", async () => {
		const fileDiff = processFile(
			"diff --git a/file.ts b/file.ts\n--- a/file.ts\n+++ b/file.ts\n@@ -100,3 +100,4 @@\n before\n-old\n+new\n+extra\n after\n",
		);
		if (!fileDiff) throw new Error("expected parsed diff");
		const item = { ...ITEM, fileDiff };
		const { host, cells, rerender } = createColumn([
			{ number: 99, type: "context-expanded" },
			{ number: 100, type: "context" },
			{ number: 101, type: "change-deletion" },
			{ number: 101 },
			{ number: 102 },
			{ number: 103, type: "context" },
			{ number: 104, type: "context-expanded" },
		]);
		const render = async () => {
			await act(async () => {
				Reflect.apply(handle().onPostRender, undefined, [
					host,
					{},
					"mount",
					{ type: "diff", item, element: host, version: item.version },
				]);
			});
		};
		const pairs = (rows: Array<HTMLElement>) =>
			rows.map((row) => [
				row.querySelector('[data-gitbutler-line-number="old"]')?.textContent,
				row.querySelector('[data-gitbutler-line-number="new"]')?.textContent,
			]);
		await render();
		const expected = [
			["99", "99"],
			["100", "100"],
			["101", "·"],
			["·", "101"],
			["·", "102"],
			["102", "103"],
			["103", "104"],
		];
		expect(pairs(cells)).toEqual(expected);
		const fresh = rerender();
		await render();
		expect(pairs(fresh)).toEqual(expected);
	});

	it("takes the actions card back when the pointer moves off the numbers onto the code", async () => {
		const { host, cell, code } = createHost();
		const context = { type: "diff", item: ITEM, element: host, version: ITEM.version };
		await act(async () => {
			Reflect.apply(handle().onPostRender, undefined, [host, {}, "mount", context]);
		});

		pointerOver(cell);
		expect(cell.querySelector("[data-gitbutler-diff-actions]")).not.toBeNull();

		pointerOver(code);
		expect(host.shadowRoot?.querySelector("[data-gitbutler-diff-actions]")).toBeNull();
	});

	describe("checkbox drag", () => {
		/** Mounts the column and presses the checkbox on `cell`. */
		const press = async (host: HTMLElement, cell: HTMLElement): Promise<void> => {
			const context = { type: "diff", item: ITEM, element: host, version: ITEM.version };
			await act(async () => {
				Reflect.apply(handle().onPostRender, undefined, [host, {}, "mount", context]);
			});
			const slot = cell.querySelector("slot[data-gitbutler-diff-gutter-slot-kind='line']");
			if (!(slot instanceof HTMLElement)) throw new Error("expected a line slot");
			slot.dispatchEvent(pointer("pointerdown"));
		};

		const paintedLines = (): Array<number | undefined> =>
			onCheckLine.mock.calls.map(([address]) => address.lineGroups[0]?.start);

		const cellAt = (cells: Array<HTMLElement>, index: number): HTMLElement => {
			const cell = cells[index];
			if (!cell) throw new Error(`expected a cell at ${index}`);
			return cell;
		};

		it("paints every line a fast drag jumps over", async () => {
			const { host, cells, pointAt } = createColumn([1, 2, 3, 4].map((number) => ({ number })));
			pointAt(cellAt(cells, 0));
			await press(host, cellAt(cells, 0));

			// The pointer outran the sampling, so the only move lands three lines down.
			pointAt(cellAt(cells, 3));
			window.dispatchEvent(pointer("pointermove"));
			window.dispatchEvent(pointer("pointerup"));

			expect(paintedLines()).toEqual([1, 2, 3, 4]);
		});

		it("paints an upward drag in the order it travelled", async () => {
			const { host, cells, pointAt } = createColumn([1, 2, 3, 4].map((number) => ({ number })));
			pointAt(cellAt(cells, 3));
			await press(host, cellAt(cells, 3));

			pointAt(cellAt(cells, 0));
			window.dispatchEvent(pointer("pointermove"));
			window.dispatchEvent(pointer("pointerup"));

			expect(paintedLines()).toEqual([4, 3, 2, 1]);
		});

		it("keeps a press that drifts onto a context line as a click", async () => {
			const { host, cells, pointAt } = createColumn([
				{ number: 1 },
				{ number: 2, type: "context" },
			]);
			pointAt(cellAt(cells, 0));
			await press(host, cellAt(cells, 0));

			pointAt(cellAt(cells, 1));
			window.dispatchEvent(pointer("pointermove"));
			window.dispatchEvent(pointer("pointerup"));

			expect(paintedLines()).toEqual([]);
			expect(host.hasAttribute("data-gitbutler-diff-check-drag")).toBe(false);
		});

		it("carries on filling after the column is re-rendered mid-drag", async () => {
			const { host, cells, pointAt, rerender } = createColumn(
				[1, 2, 3, 4].map((number) => ({ number })),
			);
			pointAt(cellAt(cells, 0));
			await press(host, cellAt(cells, 0));

			pointAt(cellAt(cells, 1));
			window.dispatchEvent(pointer("pointermove"));
			expect(paintedLines()).toEqual([1, 2]);

			const fresh = rerender();
			pointAt(cellAt(fresh, 3));
			window.dispatchEvent(pointer("pointermove"));
			window.dispatchEvent(pointer("pointerup"));

			expect(paintedLines()).toEqual([1, 2, 3, 4]);
		});
	});
});
