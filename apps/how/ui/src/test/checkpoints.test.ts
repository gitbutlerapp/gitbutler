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
});
