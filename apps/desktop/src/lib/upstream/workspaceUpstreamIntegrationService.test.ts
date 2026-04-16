import { WorkspaceUpstreamIntegrationService } from "./workspaceUpstreamIntegrationService.svelte";
import { describe, expect, test, vi } from "vitest";

describe("WorkspaceUpstreamIntegrationService", () => {
	test("requests dry-run previews with dryRun=true", async () => {
		const previewFetch = vi.fn(async () => ({ headInfo: undefined, replacedCommits: {} }));
		const backendApi = {
			endpoints: {
				workspaceHeadInfo: { useQuery: vi.fn(), fetch: vi.fn() },
				workspaceIntegrateUpstreamPreview: { fetch: previewFetch },
				workspaceIntegrateUpstream: { useMutation: vi.fn(), mutate: vi.fn() },
				deleteLocalBranch: { mutate: vi.fn() },
			},
		} as any;

		const service = new WorkspaceUpstreamIntegrationService(backendApi);
		await service.preview({
			projectId: "proj",
			updates: [{ kind: "rebase", selector: { type: "commit", subject: "abc" } }],
		});

		expect(previewFetch).toHaveBeenCalledWith(
			{
				projectId: "proj",
				updates: [{ kind: "rebase", selector: { type: "commit", subject: "abc" } }],
			},
			{ forceRefetch: true },
		);
	});

	test("fetches current head info with forceRefetch", async () => {
		const fetchHeadInfo = vi.fn(async () => ({ stacks: [] }));
		const backendApi = {
			endpoints: {
				workspaceHeadInfo: { useQuery: vi.fn(), fetch: fetchHeadInfo },
				workspaceIntegrateUpstreamPreview: { fetch: vi.fn() },
				workspaceIntegrateUpstream: { useMutation: vi.fn(), mutate: vi.fn() },
				deleteLocalBranch: { mutate: vi.fn() },
			},
		} as any;

		const service = new WorkspaceUpstreamIntegrationService(backendApi);
		await service.fetchHeadInfo("proj");

		expect(fetchHeadInfo).toHaveBeenCalledWith({ projectId: "proj" }, { forceRefetch: true });
	});

	test("exposes the workspace integration mutation hook", () => {
		const mutation = [vi.fn(), { isLoading: false }];
		const useMutation = vi.fn(() => mutation);
		const backendApi = {
			endpoints: {
				workspaceHeadInfo: { useQuery: vi.fn(), fetch: vi.fn() },
				workspaceIntegrateUpstreamPreview: { fetch: vi.fn() },
				workspaceIntegrateUpstream: { useMutation, mutate: vi.fn() },
				deleteLocalBranch: { mutate: vi.fn() },
			},
		} as any;

		const service = new WorkspaceUpstreamIntegrationService(backendApi);

		expect(service.integrateUpstream()).toBe(mutation);
		expect(useMutation).toHaveBeenCalledOnce();
	});
});
