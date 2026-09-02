/** @vitest-environment jsdom */

import { hunkAddress, type HunkAddress } from "#ui/addresses.ts";
import { store } from "#ui/store.ts";
import type { CodeViewDiffItem } from "@pierre/diffs";
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

const Probe = forwardRef<GutterHandle>((_props, ref) => {
	const result = useDiffGutterCheckboxes<unknown>(
		vi.fn(),
		() => hunkAddress(HUNK),
		() => hunkAddress(HUNK),
		"project",
		vi.fn(),
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

describe("useDiffGutterCheckboxes", () => {
	let container: HTMLDivElement;
	let root: Root;
	let handleRef: RefObject<GutterHandle | null>;

	const handle = (): GutterHandle => {
		if (!handleRef.current) throw new Error("Probe did not expose the gutter handle");
		return handleRef.current;
	};

	beforeEach(() => {
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
});
