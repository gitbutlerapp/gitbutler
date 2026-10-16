import IntegrateUpstreamModal from "$components/upstream/IntegrateUpstreamModal.svelte";
import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
import { URL_SERVICE } from "$lib/backend/url";
import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
import { UPSTREAM_INTEGRATION_SERVICE } from "$lib/upstream/upstreamIntegrationService.svelte";
import { cleanup, render, screen, waitFor } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { afterAll, afterEach, beforeAll, describe, expect, test, vi } from "vitest";

vi.mock("$lib/baseBranch/baseBranchService.svelte", async () => {
	const { InjectionToken } = await import("@gitbutler/core/context");
	return { BASE_BRANCH_SERVICE: new InjectionToken("BaseBranchService") };
});

class NoopResizeObserver {
	observe() {}
	unobserve() {}
	disconnect() {}
}

beforeAll(() => vi.stubGlobal("ResizeObserver", NoopResizeObserver));
afterAll(() => vi.unstubAllGlobals());

const statuses = {
	subject: [],
	updates: [],
	worktreeConflicts: [],
};

async function renderModal({
	loadStatuses,
	integrate = vi.fn().mockResolvedValue(undefined),
	refreshBaseBranch = vi.fn().mockResolvedValue(undefined),
}: {
	loadStatuses: ReturnType<typeof vi.fn>;
	integrate?: ReturnType<typeof vi.fn>;
	refreshBaseBranch?: ReturnType<typeof vi.fn>;
}) {
	const context = new Map<unknown, unknown>([
		[
			UPSTREAM_INTEGRATION_SERVICE._key,
			{
				upstreamStatuses: loadStatuses,
				integrateUpstream: () => [integrate, { current: { isLoading: false } }],
			},
		],
		[
			BASE_BRANCH_SERVICE._key,
			{
				baseBranch: () => ({
					response: {
						branchName: "main",
						targetShaAheadOfRef: null,
						upstreamCommits: [],
					},
				}),
				refreshBaseBranch,
			},
		],
		[FORGE_INFO_SERVICE._key, { get: () => ({ response: undefined }) }],
		[URL_SERVICE._key, { openExternalUrl: vi.fn() }],
		[CLIPBOARD_SERVICE._key, { write: vi.fn() }],
	]);
	const rendered = render(IntegrateUpstreamModal, { props: { projectId: "project-1" }, context });
	await rendered.component.show();
	return { ...rendered, integrate, refreshBaseBranch };
}

afterEach(() => {
	cleanup();
	vi.useRealTimers();
	vi.restoreAllMocks();
});

describe("IntegrateUpstreamModal failure recovery", () => {
	test("shows a retryable preview error and enables Apply only after recovery", async () => {
		vi.spyOn(console, "error").mockImplementation(() => {});
		const loadStatuses = vi
			.fn()
			.mockRejectedValueOnce(new Error("preview failed"))
			.mockResolvedValueOnce(statuses);
		await renderModal({ loadStatuses });

		expect(await screen.findByText("Couldn't preview this workspace update.")).toBeVisible();
		const apply = screen.getByRole("button", { name: "Update workspace" });
		expect(apply).toBeDisabled();
		expect(apply.querySelector(".spinner")).toBeNull();

		await userEvent.click(screen.getByRole("button", { name: "Try again" }));

		await waitFor(() => expect(loadStatuses).toHaveBeenCalledTimes(2));
		await waitFor(() =>
			expect(screen.getByRole("button", { name: "Update workspace" })).toBeEnabled(),
		);
		expect(screen.queryByText("Couldn't preview this workspace update.")).not.toBeInTheDocument();
	});

	test("re-previews before retrying execution successfully", async () => {
		vi.spyOn(console, "error").mockImplementation(() => {});
		let rejectPreview!: (error: Error) => void;
		const rePreview = new Promise<typeof statuses>((_resolve, reject) => {
			rejectPreview = reject;
		});
		const loadStatuses = vi
			.fn()
			.mockResolvedValueOnce(statuses)
			.mockReturnValueOnce(rePreview)
			.mockResolvedValueOnce(statuses);
		const integrate = vi
			.fn()
			.mockRejectedValueOnce(new Error("execution failed"))
			.mockResolvedValueOnce(undefined);
		const refreshBaseBranch = vi.fn().mockRejectedValue(new Error("refresh failed"));
		const rendered = await renderModal({ loadStatuses, integrate, refreshBaseBranch });
		const apply = await screen.findByRole("button", { name: "Update workspace" });
		await waitFor(() => expect(apply).toBeEnabled());

		await userEvent.click(apply);

		expect(await screen.findByText("Couldn't update the workspace.")).toBeVisible();
		expect(apply).toBeDisabled();
		await userEvent.click(screen.getByRole("button", { name: "Try again" }));

		await waitFor(() => expect(loadStatuses).toHaveBeenCalledTimes(2));
		expect(apply).toBeDisabled();
		rejectPreview(new Error("re-preview failed"));
		expect(await screen.findByText("Couldn't preview this workspace update.")).toBeVisible();
		expect(apply).toBeDisabled();
		await userEvent.click(screen.getByRole("button", { name: "Try again" }));

		await waitFor(() => expect(loadStatuses).toHaveBeenCalledTimes(3));
		expect(integrate).toHaveBeenCalledTimes(1);
		await waitFor(() => expect(apply).toBeEnabled());
		await userEvent.click(apply);

		await waitFor(() => expect(integrate).toHaveBeenCalledTimes(2));
		expect(rendered.refreshBaseBranch).toHaveBeenCalledWith("project-1");
		await waitFor(() =>
			expect(screen.queryByTestId("integrate-upstream-commits-modal")).not.toBeInTheDocument(),
		);
	});

	test("ignores an execution failure from a closed modal session", async () => {
		vi.spyOn(console, "error").mockImplementation(() => {});
		let rejectExecution!: (error: Error) => void;
		const execution = new Promise<void>((_resolve, reject) => {
			rejectExecution = reject;
		});
		const loadStatuses = vi.fn().mockResolvedValue(statuses);
		const rendered = await renderModal({
			loadStatuses,
			integrate: vi.fn().mockReturnValue(execution),
		});
		await waitFor(() =>
			expect(screen.getByRole("button", { name: "Update workspace" })).toBeEnabled(),
		);

		const applyClick = userEvent.click(screen.getByRole("button", { name: "Update workspace" }));
		await waitFor(() => expect(rendered.integrate).toHaveBeenCalledTimes(1));
		await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
		await waitFor(() =>
			expect(screen.queryByTestId("integrate-upstream-commits-modal")).not.toBeInTheDocument(),
		);
		vi.useFakeTimers();
		await rendered.component.show();
		await vi.advanceTimersByTimeAsync(250);
		expect(loadStatuses).toHaveBeenCalledTimes(1);
		expect(screen.getByRole("button", { name: "Update workspace" })).toBeDisabled();
		vi.useRealTimers();
		rejectExecution(new Error("stale execution failed"));
		await applyClick;

		await waitFor(() => expect(loadStatuses).toHaveBeenCalledTimes(2));
		await waitFor(() =>
			expect(screen.getByRole("button", { name: "Update workspace" })).toBeEnabled(),
		);
		expect(screen.queryByText("Couldn't update the workspace.")).not.toBeInTheDocument();
	});
});
