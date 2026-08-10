/**
 * @file React to events the backend sends about a subscribed project.
 *
 * Each event says what happened in the repository, never what the UI should do
 * about it. What it should do is derived: the event declares the tags it makes
 * stale, each query declares the tags it provides, and where they meet, the
 * query is refreshed. The two cases where invalidating is not the best move
 * are handled separately below.
 */

import {
	projectQueryKeys,
	refreshIntegratedReviews,
	type ProjectQueryKey,
} from "#ui/api/queries.ts";
import type { WatcherEvent } from "@gitbutler/but-sdk";
import { apiProvides, watcherInvalidates, type CacheTag } from "@gitbutler/but-sdk/cache-tags";
import type { QueryClient } from "@tanstack/react-query";

/** What happened in the project, without the detail the event carried. */
type ProjectEvent = WatcherEvent["payload"]["type"];

// The event table must answer for every event the backend can send.
const eventTags: Record<ProjectEvent, ReadonlyArray<CacheTag>> = watcherInvalidates;

/** The events invalidating each tag, inverted once from the event table. */
const eventsInvalidating = new Map<CacheTag, Array<ProjectEvent>>();
for (const [event, tags] of Object.entries(eventTags) as Array<
	[ProjectEvent, ReadonlyArray<CacheTag>]
>) {
	for (const tag of tags) {
		const events = eventsInvalidating.get(tag);
		if (events) events.push(event);
		else eventsInvalidating.set(tag, [event]);
	}
}

/** The queries providing a tag the event invalidates. */
type QueriesStaleAfter<Event extends ProjectEvent> = {
	[Q in keyof typeof apiProvides]: (typeof apiProvides)[Q][number] &
		(typeof watcherInvalidates)[Event][number] extends never
		? never
		: Q;
}[keyof typeof apiProvides] &
	ProjectQueryKey;

/**
 * Queries left out of `invalidateOn`, because for these `handleProjectEvent`
 * has a better answer than invalidating. Each entry says what it does instead.
 *
 * They are stale — the exception is about the remedy, not the diagnosis — so
 * the `satisfies` holds them to that: naming a query the event does not make
 * stale is a type error rather than a disagreement.
 */
const handledSeparately: Partial<Record<ProjectEvent, ReadonlySet<ProjectQueryKey>>> = {
	// Pushes instead: the event carries the new changes, so invalidating would
	// spend a round trip fetching what is already in hand.
	worktreeChanges: new Set(["changesInWorktree"]),
	// Invalidates later instead: re-reading before the review refresh lands
	// returns commits without their annotations.
	gitFetch: new Set(["workspaceTargetCommits"]),
} satisfies { [Event in ProjectEvent]?: ReadonlySet<QueriesStaleAfter<Event>> };

/**
 * The queries to invalidate when an event arrives, inverted once so an event
 * costs a lookup rather than a walk of every query.
 */
const invalidateOn = new Map<ProjectEvent, Array<ProjectQueryKey>>();
for (const query of projectQueryKeys) {
	for (const tag of apiProvides[query]) {
		for (const event of eventsInvalidating.get(tag) ?? []) {
			if (handledSeparately[event]?.has(query)) continue;
			const queries = invalidateOn.get(event);
			if (!queries) invalidateOn.set(event, [query]);
			else if (!queries.includes(query)) queries.push(query);
		}
	}
}

export const handleProjectEvent = (
	event: WatcherEvent,
	projectId: string,
	client: QueryClient,
): void => {
	const { payload } = event;

	if (payload.type === "worktreeChanges")
		client.setQueryData(["changesInWorktree", projectId], () => payload.subject.changes);

	for (const query of invalidateOn.get(payload.type) ?? [])
		void client.invalidateQueries({ queryKey: [query, projectId] });

	// The annotations read the backend's review cache, so integrated reviews have
	// to land before the listing is re-read. A failed refresh degrades to
	// unannotated commits until the next fetch.
	if (payload.type === "gitFetch") {
		void refreshIntegratedReviews(client, projectId)
			.catch(() => undefined)
			.finally(() => {
				void client.invalidateQueries({
					queryKey: ["workspaceTargetCommits", projectId],
				});
			});
	}
};
