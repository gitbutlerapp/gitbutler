import { UpstreamIntegrationService } from "$lib/upstream/upstreamIntegrationService.svelte";
import { describe, expect, test, vi } from "vitest";
import type { RefInfo, Segment, Stack } from "@gitbutler/but-sdk";

function segment(): Segment {
	return {
		refName: null,
		remoteTrackingRefName: null,
		commits: [],
		commitsOnRemote: [],
		commitsOutside: null,
		metadata: null,
		isEntrypoint: false,
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
		workspaceRef: null,
		stacks,
		target: null,
		isManagedRef: true,
		isManagedCommit: true,
		isEntrypoint: true,
	};
}

describe("UpstreamIntegrationService", () => {
	test("previews empty update sets through the backend", async () => {
		const stacks = [stack([segment()])];
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
		);

		const statuses = await service.upstreamStatuses("project-1");

		expect(mutate).toHaveBeenCalledWith({
			projectId: "project-1",
			updates: [],
			dryRun: true,
		});
		expect(statuses).toMatchObject({
			updates: [],
			worktreeConflicts: [],
			subject: [
				{
					status: "clear",
					fullyIntegrated: false,
				},
			],
		});
	});
});
