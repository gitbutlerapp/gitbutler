/** @vitest-environment jsdom */

import {
	hydratePartialDiff,
	processFile,
	type CodeViewDiffItem,
	type FileDiff,
} from "@pierre/diffs";
import { act, createRef, forwardRef, type RefObject, useImperativeHandle } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDiffSearchMarks } from "./diff-search-marks.ts";

declare global {
	var IS_REACT_ACT_ENVIRONMENT: boolean | undefined;
}

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const partialFileDiff = processFile(
	[
		"diff --git a/file.ts b/file.ts",
		"--- a/file.ts",
		"+++ b/file.ts",
		"@@ -2,1 +2,1 @@",
		"-old",
		"+new",
		"",
	].join("\n"),
	{ cacheKey: "file.ts" },
);
if (!partialFileDiff) throw new Error("Failed to parse patch");

const ITEM: CodeViewDiffItem<unknown> = {
	type: "diff",
	id: "file.ts",
	version: 1,
	fileDiff: partialFileDiff,
};
const hydratedFileDiff = hydratePartialDiff("clone", partialFileDiff, {
	oldFile: { name: "file.ts", contents: "before\nold\nafter\n" },
	newFile: { name: "file.ts", contents: "before\nnew\nafter\n" },
});

type SearchMarksHandle = ReturnType<typeof useDiffSearchMarks>;

const Probe = forwardRef<SearchMarksHandle, { items: Array<CodeViewDiffItem<unknown>> }>(
	({ items }, ref) => {
		const result = useDiffSearchMarks(vi.fn(), items);
		useImperativeHandle(ref, () => result, [result]);
		return null;
	},
);

describe("useDiffSearchMarks", () => {
	let container: HTMLDivElement;
	let root: Root;
	let resultRef: RefObject<SearchMarksHandle | null>;

	const result = (): SearchMarksHandle => {
		if (!resultRef.current) throw new Error("Probe did not expose the search marks handle");
		return resultRef.current;
	};
	const render = (items: Array<CodeViewDiffItem<unknown>>) =>
		act(() => root.render(<Probe ref={resultRef} items={items} />));

	beforeEach(() => {
		container = document.createElement("div");
		document.body.append(container);
		root = createRoot(container);
		resultRef = createRef<SearchMarksHandle>();
	});

	afterEach(() => {
		act(() => root.unmount());
		container.remove();
	});

	it("drops a cached instance when its item leaves current membership", () => {
		render([ITEM]);
		const host = document.createElement("div");
		const instance = {
			fileDiff: hydratedFileDiff,
			isLineRenderable: vi.fn(() => true),
		} as unknown as FileDiff<unknown>;
		const context = {
			type: "diff",
			item: ITEM,
			index: 0,
			top: 0,
			height: 100,
			element: host,
			version: ITEM.version,
			renderedOptionsRevision: 0,
			instance,
		};

		Reflect.apply(result().onPostRender, undefined, [host, instance, "mount", context]);
		Reflect.apply(result().onPostRender, undefined, [host, instance, "unmount", context]);

		expect(result().getSearchSource(ITEM)?.fileDiff).toBe(hydratedFileDiff);

		render([]);
		const returnedItem = { ...ITEM };
		render([returnedItem]);

		expect(result().getSearchSource(returnedItem)).toBeUndefined();
	});
});
