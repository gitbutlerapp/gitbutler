import {
	forgeHunkDiff,
	threadsByPathForScope,
	threadStillAnchored,
	threadStillAnchoredInFile,
} from "./review-threads.ts";
import { branchFileParent, commitFileParent } from "#ui/addresses.ts";
import type { ForgeReviewThread } from "@gitbutler/but-sdk";
import { DiffHunksRenderer, parsePatchFiles } from "@pierre/diffs";
import { describe, expect, it } from "vitest";

const thread = (overrides: Partial<ForgeReviewThread> = {}): ForgeReviewThread => ({
	id: "PRRT_1",
	path: "src/lib.rs",
	line: 12,
	startLine: 12,
	originalLine: 12,
	side: "new",
	isResolved: false,
	isOutdated: false,
	comments: [],
	...overrides,
});

const branch = branchFileParent({ branchRef: [] });

describe("inline thread row order", () => {
	it.each(["unified", "split"] as const)(
		"keeps %s code rows in sequence around a thread",
		async (diffStyle) => {
			const [patch] = parsePatchFiles(
				[
					"diff --git a/view.tsx b/view.tsx",
					"--- a/view.tsx",
					"+++ b/view.tsx",
					"@@ -150,4 +241,5 @@",
					" context",
					"-old one",
					"-old two",
					"+new one",
					"+new two",
					"+<div className={styles.gap}>",
					" end",
					"",
				].join("\n"),
			);
			const diff = patch?.files[0];
			if (diff === undefined) throw new Error("Missing fixture diff");
			const renderer = new DiffHunksRenderer({ diffStyle });
			try {
				const before = await renderer.asyncRender(diff);
				renderer.setLineAnnotations([{ side: "additions", lineNumber: 244, metadata: undefined }]);
				const after = await renderer.asyncRender(diff);
				for (const key of [
					"unifiedGutterAST",
					"deletionsGutterAST",
					"additionsGutterAST",
				] as const) {
					const codeRows = (result: typeof after) =>
						result[key]?.filter(
							(node) =>
								node.type === "element" && node.properties["data-column-number"] !== undefined,
						);
					expect(codeRows(after)).toEqual(codeRows(before));
				}
				const gutter = diffStyle === "unified" ? after.unifiedGutterAST : after.additionsGutterAST;
				if (gutter === undefined) throw new Error("Missing rendered gutter");
				const anchor = gutter.findIndex(
					(node) => node.type === "element" && node.properties["data-column-number"] === 244,
				);
				expect(gutter[anchor + 1]).toMatchObject({
					properties: { "data-gutter-buffer": "annotation" },
				});
				expect(gutter[anchor + 2]).toMatchObject({ properties: { "data-column-number": 245 } });
			} finally {
				renderer.cleanUp();
			}
		},
	);
});

describe("threadsByPathForScope", () => {
	it("anchors an open thread on the side the forge left it", () => {
		const byPath = threadsByPathForScope(
			[thread(), thread({ id: "PRRT_2", side: "old", line: 4 })],
			branch,
		);

		expect(byPath.get("src/lib.rs")).toMatchObject([
			{ lineNumber: 12, side: "additions" },
			{ lineNumber: 4, side: "deletions" },
		]);
	});

	it("leaves out threads the diff cannot place", () => {
		const byPath = threadsByPathForScope(
			[
				thread({ id: "resolved", isResolved: true }),
				// An outdated thread's line counts in a version of the file that
				// is no longer on screen.
				thread({ id: "outdated", isOutdated: true, line: null }),
			],
			branch,
		);

		expect(byPath.size).toBe(0);
	});

	it("places nothing outside the branch diff", () => {
		// A commit's diff numbers its own version of the file, so the forge's
		// line would land somewhere else entirely.
		const commit = commitFileParent({ commitId: "a".repeat(40), changeId: "abc" });

		expect(threadsByPathForScope([thread()], commit).size).toBe(0);
	});
});

const withHunk = (diffHunk: string): ForgeReviewThread =>
	thread({
		comments: [
			{
				id: 1,
				body: "",
				author: null,
				createdAt: null,
				modifiedAt: null,
				htmlUrl: "",
				diffHunk,
				reviewId: null,
			},
		],
	});

describe("threadStillAnchored", () => {
	const hunk = "@@ -10,3 +10,4 @@ fn main() {\n context\n+let total = 1;";

	/**
	 * Both sides are one compact array for the whole file, and the hunk says
	 * where its slice starts in each — so file line 11 is addition index 4,
	 * not index 10.
	 */
	const diff = {
		hunks: [
			{
				additionStart: 10,
				additionCount: 4,
				additionLineIndex: 3,
				deletionStart: 10,
				deletionCount: 2,
				deletionLineIndex: 1,
			},
		],
		additionLines: ["a\n", "b\n", "c\n", "context\n", "let total = 1;\n", "after\n", "tail\n"],
		deletionLines: ["x\n", "let total = 1;\n", "y\n"],
	};

	it("holds while the diff still says what the forge quoted", () => {
		expect(threadStillAnchored(withHunk(hunk), 11, "additions", diff)).toBe(true);
	});

	it("fails once that line has changed underneath", () => {
		const moved = "@@ -10,3 +10,4 @@ fn main() {\n context\n+let total = 2;";

		expect(threadStillAnchored(withHunk(moved), 11, "additions", diff)).toBe(false);
	});

	it("reads the deletions side through its own index", () => {
		// Deletion line 10 is index 1 + (10 - 10) = 1 on that side, which is
		// where the quoted line sits; line 11 is index 2, which is not.
		expect(threadStillAnchored(withHunk(hunk), 10, "deletions", diff)).toBe(true);
		expect(threadStillAnchored(withHunk(hunk), 11, "deletions", diff)).toBe(false);
	});

	it("leaves a line no hunk carries alone, since nothing is drawn there", () => {
		expect(threadStillAnchored(withHunk(hunk), 400, "additions", diff)).toBe(true);
	});

	it("holds when the forge sent no hunk, since nothing contradicts it", () => {
		expect(threadStillAnchored(thread(), 11, "additions", diff)).toBe(true);
	});

	it("quotes the last diff line, not the no-newline marker after it", () => {
		const unterminated =
			"@@ -10,3 +10,4 @@ fn main() {\n context\n+let total = 1;\n\\ No newline at end of file";

		expect(threadStillAnchored(withHunk(unterminated), 11, "additions", diff)).toBe(true);
	});
});

describe("forgeHunkDiff", () => {
	it("re-tallies the counts the forge left behind", () => {
		// The header claims six and twelve lines; three and four were sent,
		// because the hunk stops at the line the comment sits on.
		const truncated = "@@ -527,6 +527,12 @@ CREATE TABLE\n a\n b\n+c\n d";
		const diff = forgeHunkDiff("src/lib.rs", truncated);

		expect(diff?.hunks[0]?.hunkSpecs).toBe("@@ -527,3 +527,4 @@\n");
		expect(diff?.deletionLines).toEqual(["a\n", "b\n", "d\n"]);
		expect(diff?.additionLines).toEqual(["a\n", "b\n", "c\n", "d\n"]);
	});

	it("keeps a blank context line, which is a space rather than nothing", () => {
		const diff = forgeHunkDiff("f.ts", "@@ -1,9 +1,9 @@\n a\n \n+b\n");

		expect(diff?.deletionLines).toEqual(["a\n", "\n"]);
		expect(diff?.additionLines).toEqual(["a\n", "\n", "b\n"]);
	});

	it("declines anything that is not a hunk", () => {
		expect(forgeHunkDiff("f.ts", "no header here")).toBeNull();
	});

	it("keys the parsed diff by content, not file name", () => {
		// Two threads on one file: the forge truncates each hunk at its own
		// comment line, so the same path carries different lines.
		const short = forgeHunkDiff("f.ts", "@@ -1,3 +1,3 @@\n a\n+b\n");
		const long = forgeHunkDiff("f.ts", "@@ -1,3 +1,3 @@\n a\n+b\n+c\n");

		expect(short?.cacheKey).toBeDefined();
		expect(short?.cacheKey).not.toBe(long?.cacheKey);
		expect(forgeHunkDiff("f.ts", "@@ -1,3 +1,3 @@\n a\n+b\n")?.cacheKey).toBe(short?.cacheKey);
	});

	it("drops the no-newline marker a file's unterminated end carries", () => {
		const diff = forgeHunkDiff("f.ts", "@@ -1,2 +1,2 @@\n a\n+b\n\\ No newline at end of file");

		expect(diff?.hunks[0]?.hunkSpecs).toBe("@@ -1,1 +1,2 @@\n");
		expect(diff?.additionLines).toEqual(["a\n", "b\n"]);
	});
});

describe("threadStillAnchoredInFile", () => {
	const hunk = "@@ -10,3 +10,4 @@ fn main() {\n context\n+let total = 1;";
	const at = (line: number) => thread({ ...withHunk(hunk), line });

	it("holds while the file still says what the forge quoted", () => {
		expect(threadStillAnchoredInFile(at(3), "a\nb\nlet total = 1;\nd")).toBe(true);
	});

	it("fails once the line has moved underneath", () => {
		// The forge still calls this current; the branch moved without it.
		expect(threadStillAnchoredInFile(at(3), "a\nb\nlet total = 2;\nd")).toBe(false);
	});

	it("fails when the file no longer reaches the line at all", () => {
		expect(threadStillAnchoredInFile(at(9), "a\nb")).toBe(false);
	});

	it("holds when there is nothing quoted to check", () => {
		expect(threadStillAnchoredInFile(thread({ line: 3 }), "a\nb\nc")).toBe(true);
	});

	it("leaves an old-side thread to the forge's own flag", () => {
		// The quote is the pre-image, which no working file holds — a mismatch
		// here says nothing about the thread having moved.
		expect(threadStillAnchoredInFile({ ...at(3), side: "old" }, "a\nb\nsomething else\nd")).toBe(
			true,
		);
	});
});
