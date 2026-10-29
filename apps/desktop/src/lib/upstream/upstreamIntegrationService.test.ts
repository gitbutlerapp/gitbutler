import { UpstreamIntegrationService } from "$lib/upstream/upstreamIntegrationService.svelte";
import { describe, expect, test, vi } from "vitest";
import type { RefInfo, Segment, Stack } from "@gitbutler/but-sdk";

const encoder = new TextEncoder();

function bytes(value: string): number[] {
	return [...encoder.encode(value)];
}

function segment(name?: string): Segment {
	return {
		refName: name
			? {
					fullNameBytes: bytes(`refs/heads/${name}`),
					displayName: name,
				}
			: null,
		remoteTrackingRefName: null,
		commits: [],
		commitsOnRemote: [],
		metadata: null,
		pushStatus: "unpushedCommits",
		base: null,
	};
}

function stack(segments: Segment[]): Stack {
	return {
		id: "stack-1",
		base: null,
		segments,
	};
}

function refInfo(stacks: Stack[]): RefInfo {
	return {
		stacks,
		target: null,
		worktrees: [],
	};
}

describe("UpstreamIntegrationService", () => {
	test("tags execution telemetry with its phase", () => {
		const useMutation = vi.fn().mockReturnValue([]);
		const service = new UpstreamIntegrationService(
			{
				endpoints: { workspaceIntegrateUpstream: { useMutation } },
			} as any,
			{} as any,
			{} as any,
		);

		service.integrateUpstream();

		expect(useMutation).toHaveBeenCalledWith(
			expect.objectContaining({ propertiesFn: expect.any(Function) }),
		);
		expect(useMutation.mock.calls[0]?.[0]?.propertiesFn()).toEqual({ phase: "execute" });
	});

	test("previews empty update sets through the backend", async () => {
		const stacks = [stack([segment()])];
		const mutate = vi.fn().mockResolvedValue({
			workspaceState: {
				headInfo: refInfo([]),
				changes: [],
				replacedCommits: {},
			},
			worktreeConflicts: ["conflicting.txt"],
		});
		const service = new UpstreamIntegrationService(
			{
				endpoints: {
					workspaceIntegrateUpstream: {
						mutate,
					},
				},
			} as any,
			{
				fetchStacks: vi.fn().mockResolvedValue(stacks),
			} as any,
			{
				waitForRefreshes: vi.fn().mockResolvedValue(undefined),
			} as any,
		);

		const statuses = await service.upstreamStatuses("project-1");

		expect(mutate).toHaveBeenCalledWith(
			{
				projectId: "project-1",
				updates: [],
				dryRun: true,
			},
			expect.objectContaining({ propertiesFn: expect.any(Function) }),
		);
		expect(mutate.mock.calls[0]?.[1]?.propertiesFn()).toEqual({ phase: "preview" });
		expect(statuses).toMatchObject({
			updates: [],
			worktreeConflicts: ["conflicting.txt"],
			subject: [
				{
					status: "clear",
					fullyIntegrated: false,
				},
			],
		});
	});

	test("previews non-empty update sets through the backend", async () => {
		const stacks = [stack([segment("feature")])];
		const mutate = vi.fn().mockResolvedValue({
			workspaceState: {
				headInfo: refInfo([]),
				changes: [],
				replacedCommits: {},
			},
			worktreeConflicts: [],
		});
		const service = new UpstreamIntegrationService(
			{
				endpoints: {
					workspaceIntegrateUpstream: {
						mutate,
					},
				},
			} as any,
			{
				fetchStacks: vi.fn().mockResolvedValue(stacks),
			} as any,
			{
				waitForRefreshes: vi.fn().mockResolvedValue(undefined),
			} as any,
		);

		const statuses = await service.upstreamStatuses("project-1");

		expect(mutate).toHaveBeenCalledWith(
			{
				projectId: "project-1",
				updates: [
					{
						kind: "rebase",
						selector: {
							type: "referenceBytes",
							subject: bytes("refs/heads/feature"),
						},
					},
				],
				dryRun: true,
			},
			expect.objectContaining({ propertiesFn: expect.any(Function) }),
		);
		expect(statuses).toMatchObject({
			updates: [
				{
					kind: "rebase",
					selector: {
						type: "referenceBytes",
						subject: bytes("refs/heads/feature"),
					},
				},
			],
			worktreeConflicts: [],
			subject: [
				{
					status: "integrated",
					fullyIntegrated: true,
				},
			],
		});
	});

	test("waits for pending direct review refresh before previewing integration", async () => {
		const calls: string[] = [];
		// eslint-disable-next-line func-style
		let finishRefresh: () => void = () => {};
		const refresh = new Promise<void>((resolve) => {
			finishRefresh = resolve;
		});
		const mutate = vi.fn().mockImplementation(async () => {
			calls.push("preview");
			return await Promise.resolve({
				workspaceState: {
					headInfo: refInfo([]),
					changes: [],
					replacedCommits: {},
				},
				worktreeConflicts: [],
			});
		});
		const service = new UpstreamIntegrationService(
			{
				endpoints: {
					workspaceIntegrateUpstream: {
						mutate,
					},
				},
			} as any,
			{
				fetchStacks: vi.fn().mockImplementation(async () => {
					calls.push("stacks");
					return await Promise.resolve([stack([segment("feature")])]);
				}),
			} as any,
			{
				waitForRefreshes: vi.fn().mockImplementation(async () => {
					calls.push("wait");
					await refresh;
					calls.push("refreshed");
				}),
			} as any,
		);

		const statuses = service.upstreamStatuses("project-1");
		await Promise.resolve();

		expect(calls).toEqual(["wait"]);

		finishRefresh();
		await statuses;

		expect(calls).toEqual(["wait", "refreshed", "stacks", "preview"]);
	});
});
