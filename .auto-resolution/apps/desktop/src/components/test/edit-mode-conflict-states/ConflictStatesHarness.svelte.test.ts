import ConflictStatesHarness from "$components/test/edit-mode-conflict-states/ConflictStatesHarness.svelte";
import { render, screen } from "@testing-library/svelte";
import { tick } from "svelte";
import { describe, expect, test, vi } from "vitest";
import type { FileService } from "$lib/files/fileService";
import type { ConflictEntryPresence } from "@gitbutler/but-sdk";

const BOTH_SIDES: ConflictEntryPresence = { ancestor: true, ours: true, theirs: true };

function conflictContent() {
	return "before\n<<<<<<< ours\nour change\n=======\ntheir change\n>>>>>>> theirs\nafter\n";
}

function resolvedContent() {
	return "before\nour change\ntheir change\nafter\n";
}

function createFileService(contentByPath: Record<string, string>): FileService {
	return {
		readFromWorkspace: vi.fn(async (path: string) => ({
			data: { content: contentByPath[path] ?? "" },
			isLarge: false,
		})),
	} as unknown as FileService;
}

/** Wait for all pending promises and Svelte reactivity to settle. */
async function settle() {
	await tick();
	await new Promise((r) => setTimeout(r, 0));
	await tick();
}

describe("conflict state tracking", () => {
	test("initially reads all conflicted files and sets their state", async () => {
		const fileService = createFileService({
			"a.txt": conflictContent(),
			"b.txt": resolvedContent(),
		});

		render(ConflictStatesHarness, {
			props: {
				files: [
					{ path: "a.txt", conflictEntryPresence: BOTH_SIDES },
					{ path: "b.txt", conflictEntryPresence: BOTH_SIDES },
					{ path: "c.txt" }, // no conflict presence — should not be read
				],
				uncommittedResponse: [],
				fileService,
				projectId: "proj",
			},
		});

		await settle();

		expect(screen.getByTestId("file-a.txt")).toHaveAttribute("data-conflict-state", "conflicted");
		expect(screen.getByTestId("file-b.txt")).toHaveAttribute("data-conflict-state", "resolved");
		expect(screen.getByTestId("file-c.txt")).toHaveAttribute("data-conflict-state", "unknown");

		// Should only read conflicted files, not c.txt
		expect(fileService.readFromWorkspace).toHaveBeenCalledTimes(2);
		expect(fileService.readFromWorkspace).toHaveBeenCalledWith("a.txt", "proj");
		expect(fileService.readFromWorkspace).toHaveBeenCalledWith("b.txt", "proj");
	});

	test("re-reads conflicted files when uncommittedResponse changes", async () => {
		const contents: Record<string, string> = {
			"a.txt": conflictContent(),
		};

		const fileService = createFileService(contents);

		const { rerender } = render(ConflictStatesHarness, {
			props: {
				files: [{ path: "a.txt", conflictEntryPresence: BOTH_SIDES }],
				uncommittedResponse: [{ path: "a.txt" }],
				fileService,
				projectId: "proj",
			},
		});

		await settle();
		expect(screen.getByTestId("file-a.txt")).toHaveAttribute("data-conflict-state", "conflicted");
		expect(fileService.readFromWorkspace).toHaveBeenCalledTimes(1);

		// Simulate the user resolving the conflict on disk.
		// The file watcher would update uncommittedResponse with a new array reference.
		contents["a.txt"] = resolvedContent();
		await rerender({
			files: [{ path: "a.txt", conflictEntryPresence: BOTH_SIDES }],
			uncommittedResponse: [{ path: "a.txt" }], // new array reference
			fileService,
			projectId: "proj",
		});

		await settle();
		expect(screen.getByTestId("file-a.txt")).toHaveAttribute("data-conflict-state", "resolved");
		expect(fileService.readFromWorkspace).toHaveBeenCalledTimes(2);
	});

	test("does not re-read when only non-conflicted files change", async () => {
		const fileService = createFileService({
			"conflict.txt": conflictContent(),
		});

		const { rerender } = render(ConflictStatesHarness, {
			props: {
				files: [
					{ path: "conflict.txt", conflictEntryPresence: BOTH_SIDES },
					{ path: "normal.txt" },
				],
				uncommittedResponse: [{ path: "conflict.txt" }],
				fileService,
				projectId: "proj",
			},
		});

		await settle();
		expect(fileService.readFromWorkspace).toHaveBeenCalledTimes(1);

		// A non-conflicted file is added — uncommittedResponse changes but
		// the conflicted file set is the same. The effect re-runs but only
		// reads conflicted files.
		await rerender({
			files: [
				{ path: "conflict.txt", conflictEntryPresence: BOTH_SIDES },
				{ path: "normal.txt" },
				{ path: "new-normal.txt" },
			],
			uncommittedResponse: [{ path: "conflict.txt" }, { path: "new-normal.txt" }],
			fileService,
			projectId: "proj",
		});

		await settle();
		// Should re-read conflict.txt (effect re-ran) but never read normal files
		expect(fileService.readFromWorkspace).toHaveBeenCalledTimes(2);
		expect(fileService.readFromWorkspace).toHaveBeenNthCalledWith(1, "conflict.txt", "proj");
		expect(fileService.readFromWorkspace).toHaveBeenNthCalledWith(2, "conflict.txt", "proj");
	});

	test("skips non-conflicted files on re-read", async () => {
		const contents: Record<string, string> = {
			"conflict.txt": conflictContent(),
		};
		const fileService = createFileService(contents);

		const { rerender } = render(ConflictStatesHarness, {
			props: {
				files: [
					{ path: "conflict.txt", conflictEntryPresence: BOTH_SIDES },
					{ path: "normal.txt" },
				],
				uncommittedResponse: [{ path: "conflict.txt" }, { path: "normal.txt" }],
				fileService,
				projectId: "proj",
			},
		});

		await settle();
		expect(fileService.readFromWorkspace).toHaveBeenCalledTimes(1);
		expect(fileService.readFromWorkspace).toHaveBeenCalledWith("conflict.txt", "proj");

		// Trigger re-read via new response reference
		contents["conflict.txt"] = resolvedContent();
		await rerender({
			files: [{ path: "conflict.txt", conflictEntryPresence: BOTH_SIDES }, { path: "normal.txt" }],
			uncommittedResponse: [{ path: "conflict.txt" }, { path: "normal.txt" }],
			fileService,
			projectId: "proj",
		});

		await settle();
		// Should only have read conflict.txt again, never normal.txt
		expect(fileService.readFromWorkspace).toHaveBeenCalledTimes(2);
		expect(fileService.readFromWorkspace).toHaveBeenNthCalledWith(2, "conflict.txt", "proj");
	});

	test("updates state from conflicted to resolved when file is fixed", async () => {
		const contents: Record<string, string> = {
			"a.txt": conflictContent(),
			"b.txt": conflictContent(),
		};
		const fileService = createFileService(contents);

		const { rerender } = render(ConflictStatesHarness, {
			props: {
				files: [
					{ path: "a.txt", conflictEntryPresence: BOTH_SIDES },
					{ path: "b.txt", conflictEntryPresence: BOTH_SIDES },
				],
				uncommittedResponse: [],
				fileService,
				projectId: "proj",
			},
		});

		await settle();
		expect(screen.getByTestId("file-a.txt")).toHaveAttribute("data-conflict-state", "conflicted");
		expect(screen.getByTestId("file-b.txt")).toHaveAttribute("data-conflict-state", "conflicted");

		// Resolve only a.txt, leave b.txt conflicted
		contents["a.txt"] = resolvedContent();

		await rerender({
			files: [
				{ path: "a.txt", conflictEntryPresence: BOTH_SIDES },
				{ path: "b.txt", conflictEntryPresence: BOTH_SIDES },
			],
			uncommittedResponse: [{ path: "a.txt" }], // new reference triggers re-read
			fileService,
			projectId: "proj",
		});

		await settle();
		expect(screen.getByTestId("file-a.txt")).toHaveAttribute("data-conflict-state", "resolved");
		expect(screen.getByTestId("file-b.txt")).toHaveAttribute("data-conflict-state", "conflicted");
	});
});
