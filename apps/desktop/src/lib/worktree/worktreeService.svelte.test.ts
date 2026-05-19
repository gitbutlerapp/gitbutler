import {
	normalizeLocalIgnorePath,
	pathIsLocallyIgnored,
	WorktreeService,
} from "$lib/worktree/worktreeService.svelte";
import { describe, expect, test, vi } from "vitest";

describe("WorktreeService", () => {
	test("normalizes local ignore paths for matching", () => {
		expect(normalizeLocalIgnorePath(String.raw`Assets\Generated\NavMesh.asset`)).toBe(
			"Assets/Generated/NavMesh.asset",
		);
		expect(normalizeLocalIgnorePath("./Assets//Generated")).toBe("Assets/Generated");
		expect(normalizeLocalIgnorePath("../outside")).toBeUndefined();
	});

	test("matches locally ignored paths and their children", () => {
		const ignoredPaths = ["Assets/Generated", "ProjectSettings/EditorSettings.asset"];

		expect(pathIsLocallyIgnored("Assets/Generated/NavMesh.asset", ignoredPaths)).toBe(true);
		expect(pathIsLocallyIgnored("ProjectSettings/EditorSettings.asset", ignoredPaths)).toBe(true);
		expect(pathIsLocallyIgnored("Assets/Generated2/NavMesh.asset", ignoredPaths)).toBe(false);
	});

	test("setLocalIgnoredPath uses the mutation endpoint", async () => {
		const mutate = vi.fn().mockResolvedValue(undefined);
		const service = new WorktreeService({
			endpoints: {
				setLocalIgnoredPath: {
					mutate,
				},
			},
		} as never);

		await service.setLocalIgnoredPath("project-1", "Assets/Generated/NavMesh.asset", true);

		expect(mutate).toHaveBeenCalledWith({
			projectId: "project-1",
			path: "Assets/Generated/NavMesh.asset",
			ignored: true,
		});
	});

	test("serializes local ignore mutations per project", async () => {
		let releaseFirstMutation: (() => void) | undefined;
		const firstMutation = new Promise<void>((resolve) => {
			releaseFirstMutation = resolve;
		});
		const mutate = vi.fn().mockImplementationOnce(() => firstMutation).mockResolvedValue(undefined);
		const service = new WorktreeService({
			endpoints: {
				setLocalIgnoredPath: {
					mutate,
				},
			},
		} as never);

		const first = service.setLocalIgnoredPath("project-1", "Assets/Generated", true);
		const second = service.setLocalIgnoredPath("project-1", "ProjectSettings", true);

		await Promise.resolve();
		expect(mutate).toHaveBeenCalledTimes(1);

		releaseFirstMutation?.();
		await Promise.all([first, second]);

		expect(mutate).toHaveBeenCalledTimes(2);
		expect(mutate).toHaveBeenNthCalledWith(2, {
			projectId: "project-1",
			path: "ProjectSettings",
			ignored: true,
		});
	});
});
