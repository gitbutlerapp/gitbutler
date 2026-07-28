import { hasUnresolvedConflictsOnDisk } from "$lib/files/conflictCheck";
import { describe, expect, test, vi } from "vitest";
import type { FileService } from "$lib/files/fileService";
import type { ConflictEntryPresence } from "@gitbutler/but-sdk";

const BOTH_SIDES_PRESENT: ConflictEntryPresence = {
	ancestor: true,
	ours: true,
	theirs: true,
};

const OURS_DELETED: ConflictEntryPresence = {
	ancestor: true,
	ours: false,
	theirs: true,
};

function mockFileService(contentByPath: Record<string, string | null>): FileService {
	return {
		readFromWorkspace: vi.fn(async (path: string) => ({
			data: { content: path in contentByPath ? contentByPath[path] : "" },
			isLarge: false,
		})),
	} as unknown as FileService;
}

describe("hasUnresolvedConflictsOnDisk", () => {
	test("skips manually resolved files", async () => {
		const files = [
			{
				path: "a.txt",
				conflictEntryPresence: BOTH_SIDES_PRESENT,
			},
		];
		const fileService = mockFileService({
			"a.txt": "<<<<<<< still has markers\n=======\n>>>>>>> but manually resolved\n",
		});
		const manuallyResolved = new Set(["a.txt"]);

		const result = await hasUnresolvedConflictsOnDisk(files, manuallyResolved, fileService, "proj");

		expect(result).toBe(false);
		expect(fileService.readFromWorkspace).not.toHaveBeenCalled();
	});

	test("returns true for delete/modify conflicts even without markers", async () => {
		const files = [
			{
				path: "deleted.txt",
				conflictEntryPresence: OURS_DELETED,
			},
		];
		const fileService = mockFileService({
			"deleted.txt": "clean content\n",
		});

		const result = await hasUnresolvedConflictsOnDisk(files, new Set(), fileService, "proj");

		expect(result).toBe(true);
	});

	test("treats a conflicted binary file, which has no content, as not blocking", async () => {
		const files = [
			{
				path: "image.png",
				conflictEntryPresence: BOTH_SIDES_PRESENT,
			},
		];
		// Binary files come back with `content: null`, which used to reach
		// `looksConflicted` and throw "null is not an object (evaluating 's.split')".
		const fileService = mockFileService({ "image.png": null });

		const result = await hasUnresolvedConflictsOnDisk(files, new Set(), fileService, "proj");

		expect(result).toBe(false);
	});

	test("short-circuits on first conflicted file", async () => {
		const files = [
			{ path: "a.txt", conflictEntryPresence: BOTH_SIDES_PRESENT },
			{ path: "b.txt", conflictEntryPresence: BOTH_SIDES_PRESENT },
		];
		const fileService = mockFileService({
			"a.txt": "<<<<<<< conflict\n=======\n>>>>>>>\n",
			"b.txt": "resolved\n",
		});

		const result = await hasUnresolvedConflictsOnDisk(files, new Set(), fileService, "proj");

		expect(result).toBe(true);
		expect(fileService.readFromWorkspace).toHaveBeenCalledTimes(1);
	});
});
