import { describe, expect, test } from "vitest";
import type { ConflictEntryPresence, FileInfo } from "@gitbutler/but-sdk";
import { conflictHint, conflictStateOf } from "./edit-mode-conflicts.ts";

const bothSides: ConflictEntryPresence = { ancestor: true, ours: true, theirs: true };
const textFile = (content: string): FileInfo => ({
	content,
	fileName: "file.ts",
	size: content.length,
	mimeType: null,
});

describe("conflict state on disk", () => {
	test("a file still carrying a marker is conflicted", () => {
		expect(conflictStateOf(bothSides, textFile("a\n<<<<<<< ours\nb\n"))).toBe("conflicted");
	});

	test("a file the user has cleaned up is resolved", () => {
		expect(conflictStateOf(bothSides, textFile("a\nb\n"))).toBe("resolved");
	});

	test("a marker mentioned mid-line does not count", () => {
		// Only a marker git itself wrote, at the start of a line, means conflict.
		expect(conflictStateOf(bothSides, textFile('const s = "<<<<<<< ours";\n'))).toBe("resolved");
	});

	test("a file one side deleted stays conflicted, whatever its text says", () => {
		const deletedByThem: ConflictEntryPresence = { ancestor: true, ours: true, theirs: false };
		expect(conflictStateOf(deletedByThem, textFile("clean\n"))).toBe("conflicted");
	});

	test("a binary or unread file is unknown rather than resolved", () => {
		expect(conflictStateOf(bothSides, undefined)).toBe("unknown");
		expect(conflictStateOf(bothSides, { ...textFile("AAA"), mimeType: "image/png" })).toBe(
			"unknown",
		);
		expect(conflictStateOf(bothSides, { ...textFile(""), content: null })).toBe("unknown");
	});
});

describe("conflict hints", () => {
	test("names the side that deleted the file", () => {
		expect(conflictHint({ ancestor: true, ours: false, theirs: true })).toBe("deleted by you");
		expect(conflictHint({ ancestor: true, ours: true, theirs: false })).toBe("deleted by them");
		expect(conflictHint(bothSides)).toBe("conflicts");
	});
});
