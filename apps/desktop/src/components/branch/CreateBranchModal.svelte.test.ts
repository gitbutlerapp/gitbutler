import CreateBranchModal from "$components/branch/CreateBranchModal.svelte";
import { URL_SERVICE } from "$lib/backend/url";
import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
import { MODE_SERVICE } from "$lib/mode/modeService";
import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
import { cleanup, render, screen, waitFor } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, test, vi } from "vitest";

vi.mock("$lib/settings/settingsModal.svelte", () => ({
	useSettingsModal: () => ({ openGeneralSettings: vi.fn() }),
}));

vi.mock("$lib/baseBranch/baseBranchService.svelte", async () => {
	const { InjectionToken } = await import("@gitbutler/core/context");
	return { BASE_BRANCH_SERVICE: new InjectionToken("BaseBranchService") };
});

async function renderModal(
	type: "OpenWorkspace" | "OutsideWorkspace",
	switchBack: () => Promise<void>,
) {
	const createNewStack = vi.fn().mockResolvedValue(undefined);
	const context = new Map<unknown, unknown>([
		[
			STACK_SERVICE._key,
			{
				newStack: [createNewStack, { current: { isLoading: false } }],
				branchCreate: [vi.fn(), { current: { isLoading: false } }],
				stacks: () => ({ response: [] }),
				fetchNewBranchName: vi.fn().mockResolvedValue("new-branch"),
				normalizeBranchName: vi.fn().mockResolvedValue("new-branch"),
			},
		],
		[MODE_SERVICE._key, { mode: () => ({ response: { type } }) }],
		[
			BASE_BRANCH_SERVICE._key,
			{ switchBackToWorkspace: [switchBack, { current: { isLoading: false } }] },
		],
		[URL_SERVICE._key, { openExternalUrl: vi.fn() }],
	]);
	const rendered = render(CreateBranchModal, { props: { projectId: "project-1" }, context });
	await rendered.component.show();
	const submit = screen.getByRole("button", { name: "Create branch" });
	await waitFor(() => expect(submit).toBeEnabled());
	return { createNewStack, submit };
}

afterEach(cleanup);

describe("creating an independent branch", () => {
	test("waits for the workspace switch before creating the branch", async () => {
		let finishSwitch!: () => void;
		const switchBack = vi.fn().mockReturnValue(
			new Promise<void>((resolve) => {
				finishSwitch = resolve;
			}),
		);
		const { createNewStack, submit } = await renderModal("OutsideWorkspace", switchBack);

		await userEvent.click(submit);
		expect(switchBack).toHaveBeenCalledWith({ projectId: "project-1" });
		expect(createNewStack).not.toHaveBeenCalled();

		finishSwitch();
		await waitFor(() =>
			expect(createNewStack).toHaveBeenCalledWith({
				projectId: "project-1",
				branch: { name: "new-branch", order: undefined },
			}),
		);
	});

	test("does not switch when already in the workspace", async () => {
		const switchBack = vi.fn().mockResolvedValue(undefined);
		const { createNewStack, submit } = await renderModal("OpenWorkspace", switchBack);

		await userEvent.click(submit);
		expect(switchBack).not.toHaveBeenCalled();
		expect(createNewStack).toHaveBeenCalledTimes(1);
	});
});
