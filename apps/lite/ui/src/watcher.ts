import { refreshIntegratedReviews, type ProjectQueryKey } from "#ui/api/queries.ts";
import type { WatcherEvent } from "@gitbutler/but-sdk";
import type { QueryClient } from "@tanstack/react-query";

type WatcherEventType = WatcherEvent["payload"]["type"];

/**
 * A `Record`, not a `Partial`: a new project query has to declare what refreshes
 * it, or this stops compiling.
 */
const refreshedBy: Record<ProjectQueryKey, ReadonlyArray<WatcherEventType>> = {
	absorptionPlan: ["gitActivity", "workspaceActivity", "worktreeChanges"],
	// A fetch changes no local commit, but it moves remote-tracking refs.
	branchDetails: ["gitFetch", "gitActivity", "workspaceActivity"],
	branchDiff: ["gitFetch", "gitActivity", "workspaceActivity"],
	branchList: ["gitFetch", "gitActivity", "workspaceActivity"],
	// Pushed below rather than invalidated: `worktreeChanges` carries the data.
	changesInWorktree: ["gitActivity", "workspaceActivity"],
	ciChecks: [],
	commentReactions: [],
	comments: ["gitActivity", "workspaceActivity", "worktreeChanges"],
	commitDetailsWithLineStats: ["gitActivity", "workspaceActivity"],
	currentForgeLogin: [],
	dryRun: ["gitActivity", "workspaceActivity", "worktreeChanges"],
	forgeInfo: [],
	gbConfig: [],
	headInfo: ["gitActivity", "workspaceActivity"],
	repoLabels: [],
	review: ["gitFetch"],
	reviewComments: [],
	reviewMergeStatus: [],
	reviewReactions: [],
	reviewSubmissions: [],
	reviewTimelineEvents: [],
	reviewerCandidates: [],
	reviews: ["gitFetch"],
	signingSettings: [],
	treeChangeDiffs: ["gitActivity", "workspaceActivity", "worktreeChanges"],
	workspaceFetch: [],
	workspaceFetchStatus: ["gitFetch"],
	// `gitFetch` refreshes this too, but only once reviews land — see below.
	workspaceTargetCommits: ["gitActivity", "workspaceActivity"],
};

/** Inverted once, so an event costs a lookup rather than a walk of every query. */
const staleAfter = new Map<WatcherEventType, Array<ProjectQueryKey>>();
for (const [key, events] of Object.entries(refreshedBy) as Array<
	[ProjectQueryKey, ReadonlyArray<WatcherEventType>]
>) {
	for (const event of events) {
		const keys = staleAfter.get(event);
		if (keys) keys.push(key);
		else staleAfter.set(event, [key]);
	}
}

export const handleWatcher = (
	event: WatcherEvent,
	projectId: string,
	client: QueryClient,
): void => {
	const { payload } = event;

	if (payload.type === "worktreeChanges") {
		client.setQueryData(
			["changesInWorktree" satisfies ProjectQueryKey, projectId],
			() => payload.subject.changes,
		);
	}

	for (const key of staleAfter.get(payload.type) ?? [])
		void client.invalidateQueries({ queryKey: [key, projectId] });

	// The annotations read the backend's review cache, so integrated reviews have
	// to land before the listing is re-read. A failed refresh degrades to
	// unannotated commits until the next fetch.
	if (payload.type === "gitFetch") {
		void refreshIntegratedReviews(client, projectId)
			.catch(() => undefined)
			.finally(() => {
				void client.invalidateQueries({
					queryKey: ["workspaceTargetCommits" satisfies ProjectQueryKey, projectId],
				});
			});
	}
};
