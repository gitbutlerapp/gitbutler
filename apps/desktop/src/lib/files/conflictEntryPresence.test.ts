import { getConflictState } from "$lib/files/conflictEntryPresence";
import { describe, expect, test } from "vitest";
import type { ConflictEntryPresence, FileInfo } from "@gitbutler/but-sdk";

const BOTH_SIDES: ConflictEntryPresence = { ancestor: true, ours: true, theirs: true };

function fileInfo(content: string | null, mimeType: string | null = null): FileInfo {
	return { content, fileName: "file.txt", size: null, mimeType };
}

describe("getConflictState", () => {
	test("delete/modify conflicts are conflicted regardless of content", () => {
		const oursDeleted: ConflictEntryPresence = { ancestor: true, ours: false, theirs: true };
		expect(getConflictState(oursDeleted, fileInfo(null))).toBe("conflicted");
	});

	test("content with conflict markers is conflicted", () => {
		expect(getConflictState(BOTH_SIDES, fileInfo("<<<<<<< ours\n=======\n>>>>>>>\n"))).toBe(
			"conflicted",
		);
	});

	test("clean text is resolved", () => {
		expect(getConflictState(BOTH_SIDES, fileInfo("all good\n"))).toBe("resolved");
	});

	test("a marker mid-line does not count as conflicted", () => {
		expect(getConflictState(BOTH_SIDES, fileInfo('const s = "<<<<<<<";\n'))).toBe("resolved");
	});

	test("null content (non-UTF-8 or binary) is unknown", () => {
		expect(getConflictState(BOTH_SIDES, fileInfo(null))).toBe("unknown");
	});

	test("base64 image content is unknown, not scanned as text", () => {
		expect(getConflictState(BOTH_SIDES, fileInfo("aGVsbG8=", "image/png"))).toBe("unknown");
	});

	test("a file that could not be read is unknown", () => {
		expect(getConflictState(BOTH_SIDES, undefined)).toBe("unknown");
	});
});
