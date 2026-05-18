import IntegrateUpstreamModal from "$components/upstream/IntegrateUpstreamModal.svelte";
import { BACKEND } from "$lib/backend";
import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
import { URL_SERVICE } from "$lib/backend/url";
import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
import { UPSTREAM_INTEGRATION_SERVICE } from "$lib/upstream/upstreamIntegrationService.svelte";
import { render, screen, waitFor } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

const injectMap = new Map<unknown, unknown>();

class ResizeObserverMock {
	observe() {}
	unobserve() {}
	disconnect() {}
}

vi.stubGlobal("ResizeObserver", ResizeObserverMock);

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
	injectOptional(token: { _key: symbol }) {
		return injectMap.get(token);
	},
}));

describe("IntegrateUpstreamModal", () => {
	test("shows workspace update progress and transfer speed while integrating upstream", async () => {
		let progressHandler:
			| ((event: {
					payload: {
						direction: string;
						currentFile: number;
						totalFiles: number;
						fileDownloadedBytes: number;
						fileTotalBytes: number;
						progressPercent: number;
						bytesPerSecond?: number;
						path: string;
					};
			  }) => void)
			| undefined;
		let gitOperationProgressHandler:
			| ((event: {
					payload: {
						operation: string;
						phase: string;
						phaseLabel: string;
						elapsedMs: number;
						detail?: string;
					};
			  }) => void)
			| undefined;
		const listen = vi.fn((event: string, handler: typeof progressHandler) => {
			if (event.endsWith("/workspace_update_progress")) {
				progressHandler = handler;
			} else {
				gitOperationProgressHandler = handler as typeof gitOperationProgressHandler;
			}
			return async () => {};
		});

		let resolveIntegration: (() => void) | undefined;
		const integrateMutation = vi.fn(
			async () =>
				await new Promise<void>((resolve) => {
					resolveIntegration = resolve;
				}),
		);

		injectMap.set(BACKEND, { listen });
		injectMap.set(BASE_BRANCH_SERVICE, {
			baseBranch: () => ({ response: undefined }),
			refreshBaseBranch: vi.fn().mockResolvedValue(undefined),
		});
		injectMap.set(DEFAULT_FORGE_FACTORY, {
			current: {
				commitUrl: () => undefined,
			},
		});
		injectMap.set(UPSTREAM_INTEGRATION_SERVICE, {
			upstreamStatuses: vi.fn().mockResolvedValue({ type: "upToDate" }),
			integrateUpstream: () => [integrateMutation],
			resolveUpstreamIntegrationMutation: vi.fn(),
		});
		injectMap.set(URL_SERVICE, { openExternalUrl: vi.fn() });
		injectMap.set(CLIPBOARD_SERVICE, { write: vi.fn() });

		const user = userEvent.setup();
		const { component } = render(IntegrateUpstreamModal, {
			props: {
				projectId: "project-1",
			},
		});

		await (component as { show: () => Promise<void> }).show();
		await user.click(await screen.findByRole("button", { name: "Update workspace" }));
		expect(await screen.findByRole("button", { name: "Updating workspace…" })).toBeInTheDocument();
		expect(
			screen.getByText("Preparing workspace update. Waiting for Git progress."),
		).toBeInTheDocument();

		await waitFor(() =>
			expect(listen).toHaveBeenCalledWith(
				"project://project-1/workspace_update_progress",
				expect.any(Function),
			),
		);
		await waitFor(() =>
			expect(listen).toHaveBeenCalledWith(
				"project://project-1/git_operation_progress",
				expect.any(Function),
			),
		);

		gitOperationProgressHandler?.({
			payload: {
				operation: "upstreamIntegration",
				phase: "treeMerge",
				phaseLabel: "Integrating upstream changes",
				elapsedMs: 4000,
				detail: "Git LFS hydration is deferred for this operation.",
			},
		});

		expect(await screen.findByText("Integrating upstream changes")).toBeInTheDocument();
		expect(
			screen.getByText("Git LFS hydration is deferred for this operation."),
		).toBeInTheDocument();

		progressHandler?.({
			payload: {
				direction: "download",
				currentFile: 3,
				totalFiles: 12,
				fileDownloadedBytes: 2 * 1024 * 1024,
				fileTotalBytes: 4 * 1024 * 1024,
				progressPercent: 60,
				bytesPerSecond: 12 * 1024 * 1024,
				path: "Assets/Bundles/world.bundle",
			},
		});

		expect(await screen.findByText("File 3 of 12")).toBeInTheDocument();
		expect(screen.getByText("60%")).toBeInTheDocument();
		expect(screen.getByText("12.0 MB/s")).toBeInTheDocument();
		expect(screen.getByText("Assets/Bundles/world.bundle")).toBeInTheDocument();
		expect(
			screen.getByText("Downloading file 3 of 12 at 12.0 MB/s. 60% complete."),
		).toBeInTheDocument();

		resolveIntegration?.();
	});
});
