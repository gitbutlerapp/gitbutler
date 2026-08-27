import { projectQueryKeys } from "#ui/api/query-keys.ts";
import { handleProjectEvent } from "#ui/project-events.ts";
import type { WatcherEvent } from "@gitbutler/but-sdk";
import { apiProvides, watcherInvalidates } from "@gitbutler/but-sdk/cache-tags";
import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

const provides: Record<string, ReadonlyArray<string> | undefined> = apiProvides;
const eventTags: Record<string, ReadonlyArray<string>> = watcherInvalidates;

/** The queries `handleProjectEvent` invalidates, and the ones it pushes. */
const react = (
	event: string,
	{
		cachedRevision = null,
		eventRevision = null,
	}: { cachedRevision?: string | null; eventRevision?: string | null } = {},
) => {
	const invalidated: Array<unknown> = [];
	const pushed: Array<unknown> = [];
	const client = {
		invalidateQueries: ({ queryKey }: { queryKey: ReadonlyArray<unknown> }) => {
			invalidated.push(queryKey[1]);
			return Promise.resolve();
		},
		setQueryData: (queryKey: ReadonlyArray<unknown>) => pushed.push(queryKey[1]),
		getQueryData: () => ({ workspaceRevision: cachedRevision }),
		getMutationCache: () => ({ getAll: () => [] }),
		fetchQuery: () => Promise.reject(new Error("offline")),
	} as unknown as QueryClient;

	const subject =
		event === "worktreeChanges"
			? { changes: {} }
			: event === "gitActivity"
				? { headSha: "a".repeat(40), workspaceRevision: eventRevision }
				: event === "workspaceActivity"
					? { workspaceRevision: eventRevision }
					: null;
	handleProjectEvent(
		{ name: event, payload: { type: event, subject } } as WatcherEvent,
		"p1",
		client,
	);
	return { invalidated, pushed };
};

describe("tags declared in Rust", () => {
	// Guards the generated map: if it ever arrives empty, every query silently
	// stops refreshing and nothing else here would notice.
	it("answers for most of the queries", () => {
		expect(projectQueryKeys.length).toBeGreaterThan(20);
	});

	it.each(
		projectQueryKeys.filter((query) =>
			(provides[query] ?? []).some((tag) =>
				Object.values(eventTags).some((tags) => tags.includes(tag)),
			),
		),
	)("refreshes %s after each event invalidating its tags", async (query) => {
		const tags = provides[query] ?? [];
		for (const [event, invalidatedTags] of Object.entries(eventTags)) {
			if (!tags.some((tag) => invalidatedTags.includes(tag))) continue;
			const { invalidated, pushed } = react(event);
			// Some are refreshed after an await rather than straight away.
			await new Promise((resolve) => setTimeout(resolve, 0));
			expect([...invalidated, ...pushed], `after ${event}`).toContain(query);
		}
	});
});

describe("handled separately", () => {
	it.each(["gitActivity", "workspaceActivity"])(
		"keeps cached headInfo after matching %s",
		(event) => {
			const { invalidated } = react(event, {
				cachedRevision: "workspace-v1:same",
				eventRevision: "workspace-v1:same",
			});
			expect(invalidated).not.toContain("headInfo");
			expect(invalidated).toContain("branchList");
		},
	);

	it.each([
		["a different revision", "workspace-v1:old", "workspace-v1:new"],
		["a missing cached revision", null, "workspace-v1:new"],
		["a missing event revision", "workspace-v1:old", null],
	] as const)("re-reads headInfo after %s", (_case, cachedRevision, eventRevision) => {
		const { invalidated } = react("workspaceActivity", { cachedRevision, eventRevision });
		expect(invalidated).toContain("headInfo");
	});

	it("waits for a pending workspace mutation before comparing revisions", async () => {
		const client = new QueryClient();
		const queryKey = ["headInfo", "p1"] as const;
		client.setQueryData(queryKey, { headInfo: {}, workspaceRevision: "workspace-v1:old" });
		let finishMutation!: () => void;
		const mutation = client.getMutationCache().build(client, {
			mutationFn: () => new Promise<void>((resolve) => (finishMutation = resolve)),
			meta: { updatesWorkspace: true },
			onSuccess: () =>
				client.setQueryData(queryKey, {
					headInfo: {},
					workspaceRevision: "workspace-v1:new",
				}),
		});
		const execution = mutation.execute({ projectId: "p1" });
		await Promise.resolve();

		handleProjectEvent(
			{
				name: "workspaceActivity",
				payload: {
					type: "workspaceActivity",
					subject: { workspaceRevision: "workspace-v1:new" },
				},
			},
			"p1",
			client,
		);
		expect(client.getQueryState(queryKey)?.isInvalidated).toBe(false);

		finishMutation();
		await execution;
		expect(client.getQueryState(queryKey)?.isInvalidated).toBe(false);
	});

	it("waits for a pending generic operation before comparing revisions", async () => {
		const client = new QueryClient();
		const queryKey = ["headInfo", "p1"] as const;
		client.setQueryData(queryKey, { headInfo: {}, workspaceRevision: "workspace-v1:old" });
		let finishMutation!: () => void;
		const mutation = client.getMutationCache().build(client, {
			meta: { updatesWorkspace: true, projectId: "p1" },
			mutationFn: () => new Promise<void>((resolve) => (finishMutation = resolve)),
			onSuccess: () =>
				client.setQueryData(queryKey, {
					headInfo: {},
					workspaceRevision: "workspace-v1:new",
				}),
		});
		const execution = mutation.execute({ type: "moveCommit" });
		await Promise.resolve();

		handleProjectEvent(
			{
				name: "workspaceActivity",
				payload: {
					type: "workspaceActivity",
					subject: { workspaceRevision: "workspace-v1:new" },
				},
			},
			"p1",
			client,
		);
		expect(client.getQueryState(queryKey)?.isInvalidated).toBe(false);

		finishMutation();
		await execution;
		expect(client.getQueryState(queryKey)?.isInvalidated).toBe(false);
	});

	it("invalidates after a pending mutation settles at a different revision", async () => {
		const client = new QueryClient();
		const queryKey = ["headInfo", "p1"] as const;
		client.setQueryData(queryKey, { headInfo: {}, workspaceRevision: "workspace-v1:old" });
		let finishMutation!: () => void;
		const mutation = client.getMutationCache().build(client, {
			mutationFn: () => new Promise<void>((resolve) => (finishMutation = resolve)),
			meta: { updatesWorkspace: true },
			onSuccess: () =>
				client.setQueryData(queryKey, {
					headInfo: {},
					workspaceRevision: "workspace-v1:mutation",
				}),
		});
		const execution = mutation.execute({ projectId: "p1" });
		await Promise.resolve();

		handleProjectEvent(
			{
				name: "workspaceActivity",
				payload: {
					type: "workspaceActivity",
					subject: { workspaceRevision: "workspace-v1:external" },
				},
			},
			"p1",
			client,
		);
		finishMutation();
		await execution;

		expect(client.getQueryState(queryKey)?.isInvalidated).toBe(true);
	});

	it("pushes worktree changes rather than invalidating them", () => {
		const { invalidated, pushed } = react("worktreeChanges");
		expect(pushed).toEqual(["changesInWorktree"]);
		expect(invalidated).not.toContain("changesInWorktree");
	});

	it("still re-reads target commits after a fetch, once reviews have landed", async () => {
		const { invalidated } = react("gitFetch");
		expect(invalidated).not.toContain("workspaceTargetCommits");
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(invalidated).toContain("workspaceTargetCommits");
	});
});
