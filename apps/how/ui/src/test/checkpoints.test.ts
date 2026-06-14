import {
	capDiffForSummary,
	checkpointMessageFromSummary,
	parseCheckpointSummary,
} from "../../../electron/src/checkpoint-summarizer";
import { checkpointMessage, createDiffSpec } from "../../../electron/src/checkpoints";
import { describe, expect, test } from "vitest";
import type { TreeChange } from "@gitbutler/but-sdk";

describe("checkpoint helpers", () => {
	test("creates whole-file diff specs from regular changes", () => {
		const change: TreeChange = {
			path: "src/app.ts",
			pathBytes: [115, 114, 99, 47, 97, 112, 112, 46, 116, 115],
			status: {
				type: "Modification",
				subject: {
					previousState: { id: "a", kind: "Blob" },
					state: { id: "b", kind: "Blob" },
					flags: null,
				},
			},
		};

		expect(createDiffSpec(change)).toEqual({
			pathBytes: change.pathBytes,
			previousPathBytes: null,
			hunkHeaders: [],
		});
	});

	test("keeps the previous path for renames", () => {
		const change: TreeChange = {
			path: "new.ts",
			pathBytes: [110, 101, 119, 46, 116, 115],
			status: {
				type: "Rename",
				subject: {
					previousPath: "old.ts",
					previousPathBytes: [111, 108, 100, 46, 116, 115],
					previousState: { id: "a", kind: "Blob" },
					state: { id: "b", kind: "Blob" },
					flags: null,
				},
			},
		};

		expect(createDiffSpec(change).previousPathBytes).toEqual([111, 108, 100, 46, 116, 115]);
	});

	test("formats checkpoint messages with a stable prefix", () => {
		expect(checkpointMessage(new Date("2026-06-12T12:34:00Z"))).toMatch(/^Checkpoint: /);
	});

	test("uses the first summary line as the checkpoint title", () => {
		expect(parseCheckpointSummary("Adds settings\nStores debounce choices")).toEqual({
			title: "Adds settings",
			body: "Stores debounce choices",
		});
	});

	test("strips an accidental checkpoint prefix from AI titles", () => {
		expect(parseCheckpointSummary("Checkpoint: Adds settings\n\nStores debounce choices")).toEqual({
			title: "Adds settings",
			body: "Stores debounce choices",
		});
	});

	test("ignores empty AI summaries", () => {
		expect(parseCheckpointSummary("\n\n")).toBeNull();
	});

	test("builds checkpoint messages from parsed summaries", () => {
		expect(
			checkpointMessageFromSummary(new Date("2026-06-12T12:34:00Z"), {
				title: "Adds settings",
				body: "Stores debounce choices",
			}),
		).toBe("Checkpoint: Adds settings\n\nStores debounce choices");
	});

	test("falls back to date checkpoint messages without a usable summary", () => {
		expect(checkpointMessageFromSummary(new Date("2026-06-12T12:34:00Z"), null)).toMatch(
			/^Checkpoint: /,
		);
	});

	test("caps summary diffs", () => {
		const capped = capDiffForSummary("a".repeat(41 * 1024));
		expect(capped.truncated).toBe(true);
		expect(Buffer.byteLength(capped.diff, "utf8")).toBeLessThanOrEqual(40 * 1024);
	});
});
