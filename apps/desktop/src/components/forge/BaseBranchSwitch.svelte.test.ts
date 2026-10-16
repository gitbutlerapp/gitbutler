import BaseBranchSwitch from "$components/forge/BaseBranchSwitch.svelte";
import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { afterAll, beforeAll, expect, test, vi } from "vitest";

class NoopResizeObserver {
	observe() {}
	unobserve() {}
	disconnect() {}
}

beforeAll(() => vi.stubGlobal("ResizeObserver", NoopResizeObserver));
afterAll(() => vi.unstubAllGlobals());

vi.mock("$lib/baseBranch/baseBranchService.svelte", async () => {
	const { InjectionToken } = await import("@gitbutler/core/context");
	return { BASE_BRANCH_SERVICE: new InjectionToken("BaseBranchService") };
});

vi.mock("$lib/stacks/stackService.svelte", async () => {
	const { InjectionToken } = await import("@gitbutler/core/context");
	return { STACK_SERVICE: new InjectionToken("StackService") };
});

function renderSwitch(remoteBranches = [{ name: "origin/main" }]) {
	const setTargetRef = vi.fn();
	const context = new Map<any, any>([
		[
			BASE_BRANCH_SERVICE._key,
			{
				baseBranch: () => ({ response: { branchName: "origin/main", pushRemoteName: "origin" } }),
				remoteBranches: () => ({ response: remoteBranches, result: { isSuccess: true } }),
				localTargetBranches: () => ({
					response: [
						{ name: "release", refName: "refs/heads/release", isTarget: true },
						{ name: "next", refName: "refs/heads/next", isTarget: false },
					],
					result: { isSuccess: true },
				}),
				setTargetRef: [setTargetRef, { current: { isLoading: false } }],
			},
		],
		[STACK_SERVICE._key, { stacks: () => ({ response: [] }) }],
	]);
	render(BaseBranchSwitch, { props: { projectId: "project-id" }, context });
	return setTargetRef;
}

test("uses the configured local target even when its upstream has a different name", () => {
	renderSwitch();
	expect(screen.getByDisplayValue("release")).toBeVisible();
	expect(screen.getByRole("button", { name: "Update configuration" })).toBeDisabled();
});

test("switches to a local ref without using the publication remote as its prefix", async () => {
	const setTargetRef = renderSwitch();
	const user = userEvent.setup();
	await user.click(screen.getByDisplayValue("release"));
	await user.click(screen.getByText("next", { exact: true }));
	await user.click(screen.getByRole("button", { name: "Update configuration" }));
	expect(setTargetRef).toHaveBeenCalledWith({
		projectId: "project-id",
		targetRef: "refs/heads/next",
		pushRemote: "origin",
	});
});

test("offers local targets without remote branches", () => {
	renderSwitch([]);
	expect(screen.getByDisplayValue("release")).toBeVisible();
});
