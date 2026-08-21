import { buildStackEndpoints } from "$lib/stacks/stackEndpoints";
import { ReduxTag } from "$lib/state/tags";
import { configureStore } from "@reduxjs/toolkit";
import { createApi, type BaseQueryFn } from "@reduxjs/toolkit/query";
import { expect, test, vi } from "vitest";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type { RefInfo, WorkspaceIntegrateUpstreamOutcome } from "@gitbutler/but-sdk";

const projectId = "project-1";
const removed = "removed";
const surviving = "LIST";
const sameStack = "same-stack";
const otherStack = "other-stack";
const headInfo = {
	stacks: [
		{
			id: "stack-1",
			base: null,
			segments: [
				{ refName: null },
				{ refName: { displayName: surviving, fullNameBytes: [] } },
				{ refName: { displayName: sameStack, fullNameBytes: [] } },
				{ refName: { displayName: surviving, fullNameBytes: [] } },
			],
		},
		{
			id: "stack-2",
			base: null,
			segments: [{ refName: { displayName: otherStack, fullNameBytes: [] } }],
		},
	],
} as RefInfo;
const integrationResult = {
	workspaceState: { headInfo, replacedCommits: {}, checkoutConflictOccurred: false },
	worktreeConflicts: [],
} satisfies WorkspaceIntegrateUpstreamOutcome;

type TestBaseQuery = BaseQueryFn<unknown, unknown, string, { command?: string }>;

function setup() {
	const calls: { command: string | undefined; args: unknown }[] = [];
	async function baseQuery(
		args: Parameters<TestBaseQuery>[0],
		_api: Parameters<TestBaseQuery>[1],
		extraOptions: Parameters<TestBaseQuery>[2],
	): Promise<Awaited<ReturnType<TestBaseQuery>>> {
		calls.push({ command: extraOptions.command, args });
		return {
			data:
				extraOptions.command === "workspace_integrate_upstream"
					? integrationResult
					: extraOptions.command === "branch_land"
						? { workspace: { headInfo } }
						: { changes: [], stats: { linesAdded: 0, linesRemoved: 0, filesChanged: 0 } },
		};
	}
	const api = createApi({
		baseQuery,
		reducerPath: "stackInvalidationApi",
		tagTypes: Object.values(ReduxTag),
		invalidationBehavior: "immediately",
		endpoints: (build) => buildStackEndpoints(build as BackendEndpointBuilder),
	});
	const store = configureStore({
		reducer: { [api.reducerPath]: api.reducer },
		middleware: (getDefaultMiddleware) => getDefaultMiddleware().concat(api.middleware),
	});
	return { api, calls, store };
}

function branchDiffCalls(calls: ReturnType<typeof setup>["calls"], branch: string) {
	return calls.filter(
		(call) =>
			call.command === "branch_diff" &&
			(call.args as { branch?: string } | undefined)?.branch === branch,
	).length;
}

test("integration refetches every surviving branch without refetching the removed branch", async () => {
	const { api, calls, store } = setup();
	const unsubscribe: (() => void)[] = [];
	const pending: PromiseLike<unknown>[] = [];
	for (const branch of [removed, surviving, sameStack, otherStack]) {
		const subscription = store.dispatch(
			api.endpoints.branchChanges.initiate({ projectId, branch }),
		);
		pending.push(subscription);
		unsubscribe.push(() => subscription.unsubscribe());
	}
	await Promise.all(pending);

	await store.dispatch(
		api.endpoints.workspaceIntegrateUpstream.initiate({ projectId, updates: [], dryRun: false }),
	);

	await vi.waitFor(() => {
		expect(branchDiffCalls(calls, removed)).toBe(1);
		expect(branchDiffCalls(calls, surviving)).toBe(2);
		expect(branchDiffCalls(calls, sameStack)).toBe(2);
		expect(branchDiffCalls(calls, otherStack)).toBe(2);
	});

	await store.dispatch(
		api.endpoints.landBranch.initiate({
			projectId,
			branch: removed,
			noFf: false,
			wholeStack: false,
		}),
	);
	await vi.waitFor(() => {
		expect(branchDiffCalls(calls, removed)).toBe(1);
		expect(branchDiffCalls(calls, surviving)).toBe(3);
		expect(branchDiffCalls(calls, sameStack)).toBe(3);
		expect(branchDiffCalls(calls, otherStack)).toBe(3);
	});
	unsubscribe.forEach((dispose) => dispose());
});
