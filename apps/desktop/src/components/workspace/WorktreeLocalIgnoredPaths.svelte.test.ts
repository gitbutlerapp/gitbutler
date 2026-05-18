import WorktreeLocalIgnoredPaths from "$components/workspace/WorktreeLocalIgnoredPaths.svelte";
import { WORKTREE_SERVICE } from "$lib/worktree/worktreeService.svelte";
import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

const injectMap = new Map<unknown, unknown>();

vi.mock("@gitbutler/core/context", () => ({
	InjectionToken: class {
		_key = Symbol();
	},
	inject(token: { _key: symbol }) {
		const value = injectMap.get(token);
		if (!value) {
			throw new Error("No mock for token");
		}
		return value;
	},
}));

describe("WorktreeLocalIgnoredPaths", () => {
	test("renders locally ignored paths and lets the user unignore them", async () => {
		const setLocalIgnoredPath = vi.fn().mockResolvedValue(undefined);
		injectMap.set(WORKTREE_SERVICE, {
			localIgnoredPaths: () => ({
				response: ["Assets/Generated/NavMesh.asset", "Assets/Scenes/dealers/LightingData.asset"],
			}),
			setLocalIgnoredPath,
		});

		const user = userEvent.setup();
		render(WorktreeLocalIgnoredPaths, {
			props: {
				projectId: "project-1",
			},
		});

		expect(screen.getByText("Locally ignored")).toBeInTheDocument();
		expect(screen.getByText("Assets/Generated/NavMesh.asset")).toBeInTheDocument();
		expect(screen.getByText("Assets/Scenes/dealers/LightingData.asset")).toBeInTheDocument();

		await user.click(
			screen.getByRole("button", { name: "Stop ignoring Assets/Generated/NavMesh.asset" }),
		);

		expect(setLocalIgnoredPath).toHaveBeenCalledWith(
			"project-1",
			"Assets/Generated/NavMesh.asset",
			false,
		);
	});
});
