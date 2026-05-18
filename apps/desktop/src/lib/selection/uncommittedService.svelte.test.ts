import { uncommittedSlice } from "$lib/selection/uncommitted";
import { UncommittedService } from "$lib/selection/uncommittedService.svelte";
import { tick } from "svelte";
import { describe, expect, test, vi } from "vitest";
import type { HunkAssignment, TreeChange } from "@gitbutler/but-sdk";

function bytes(path: string): number[] {
	return Array.from(new TextEncoder().encode(path));
}

function wholeFileAssignment(path: string): HunkAssignment {
	return {
		id: path,
		hunkHeader: null,
		path,
		pathBytes: bytes(path),
		stackId: null,
		branchRefBytes: null,
		lineNumsAdded: null,
		lineNumsRemoved: null,
	};
}

function treeChange(path: string): TreeChange {
	return {
		path,
		pathBytes: bytes(path),
		status: {
			type: "Addition",
			subject: {
				isUntracked: false,
				state: {
					id: "0000000000000000000000000000000000000000",
					kind: "Blob",
				},
			},
		},
	};
}

describe("UncommittedService", () => {
	test("persists only hunk selections", () => {
		const injectPersistedSlice = vi.fn(() => () => undefined);

		$effect.root(() => {
			new UncommittedService(
				{
					dispatch: vi.fn(),
					injectPersistedSlice,
				} as never,
				{} as never,
				{} as never,
			);
		});

		expect(injectPersistedSlice).toHaveBeenCalledWith(uncommittedSlice, {
			blacklist: ["treeChanges", "hunkAssignments"],
		});
	});

	test("does not fetch diffs for whole-file selections", async () => {
		const path = "src/a.ts";
		let state = uncommittedSlice.reducer(
			undefined,
			uncommittedSlice.actions.update({
				assignments: [wholeFileAssignment(path)],
				changes: [treeChange(path)],
			}),
		);
		state = uncommittedSlice.reducer(
			state,
			uncommittedSlice.actions.checkFiles({ stackId: null, paths: [path] }),
		);
		const fetchDiff = vi.fn();
		let service!: UncommittedService;

		const cleanup = $effect.root(() => {
			service = new UncommittedService(
				{
					dispatch: vi.fn(),
					injectPersistedSlice: vi.fn(() => () => state),
				} as never,
				{} as never,
				{ fetchDiff } as never,
			);
		});

		await tick();
		const changes = await service.worktreeChanges("project-id");
		cleanup();

		expect(fetchDiff).not.toHaveBeenCalled();
		expect(changes).toEqual([
			{
				pathBytes: bytes(path),
				previousPathBytes: null,
				hunkHeaders: [],
			},
		]);
	});
});
