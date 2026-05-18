import { uncommittedSlice } from "$lib/selection/uncommitted";
import { UncommittedService } from "$lib/selection/uncommittedService.svelte";
import { describe, expect, test, vi } from "vitest";

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
});
