import { WorktreeService } from "$lib/worktree/worktreeService.svelte";
import { describe, expect, test, vi } from "vitest";

describe("WorktreeService", () => {
	test("setLocalIgnoredPath uses the mutation endpoint", async () => {
		const mutate = vi.fn().mockResolvedValue(undefined);
		const service = new WorktreeService({
			endpoints: {
				setLocalIgnoredPath: {
					mutate,
				},
			},
		} as never);

		await service.setLocalIgnoredPath(
			"project-1",
			"Assets/Generated/NavMesh.asset",
			true,
		);

		expect(mutate).toHaveBeenCalledWith({
			projectId: "project-1",
			path: "Assets/Generated/NavMesh.asset",
			ignored: true,
		});
	});
});
