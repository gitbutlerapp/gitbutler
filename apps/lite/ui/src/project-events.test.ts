import { projectQueryKeys } from "#ui/api/queries.ts";
import { handleProjectEvent } from "#ui/project-events.ts";
import type { WatcherEvent } from "@gitbutler/but-sdk";
import { apiProvides, watcherInvalidates } from "@gitbutler/but-sdk/cache-tags";
import type { QueryClient } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

const provides: Record<string, ReadonlyArray<string> | undefined> = apiProvides;
const eventTags: Record<string, ReadonlyArray<string>> = watcherInvalidates;

/** The queries `handleProjectEvent` invalidates, and the ones it pushes. */
const react = (event: string) => {
	const invalidated: Array<unknown> = [];
	const pushed: Array<unknown> = [];
	const client = {
		invalidateQueries: ({ queryKey }: { queryKey: ReadonlyArray<unknown> }) => {
			invalidated.push(queryKey[0]);
			return Promise.resolve();
		},
		setQueryData: (queryKey: ReadonlyArray<unknown>) => pushed.push(queryKey[0]),
		fetchQuery: () => Promise.reject(new Error("offline")),
	} as unknown as QueryClient;

	const subject = event === "worktreeChanges" ? { changes: {} } : null;
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
