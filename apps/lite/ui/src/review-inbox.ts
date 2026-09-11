import {
	capInboxEntries,
	reviewStateQueryOptions,
	updateReviewState,
	type InboxEntry,
	type InboxKind,
} from "#ui/review-state.ts";
export { isBotEntry, type InboxEntry, type InboxKind } from "#ui/review-state.ts";
import type { ShowNotificationParams } from "#electron/ipc.ts";
import { reviewsSeenUpTo } from "#ui/review-seen.ts";
import { useQuery, type QueryClient } from "@tanstack/react-query";

/**
 * Loud kinds have a human waiting on the user; quiet ones are news the bell
 * keeps. The split sets a row's weight and decides what reaches the desktop.
 */
export const inboxKindAttention: Record<InboxKind, "loud" | "quiet"> = {
	comment: "loud",
	mention: "loud",
	approved: "loud",
	changesRequested: "loud",
	reviewRequested: "loud",
	committed: "quiet",
	merged: "quiet",
	closed: "quiet",
};

/** What happened, and by whom — "Changes requested by alice". */
export const entryHeadline = (entry: InboxEntry): string => {
	const by = entry.author === null ? "" : ` by ${entry.author}`;
	const from = entry.author === null ? "" : ` from ${entry.author}`;
	const many = entry.count > 1;
	switch (entry.kind) {
		case "comment":
			return many ? `${entry.count} comments${from}` : `Comment${from}`;
		case "mention":
			return `Mentioned${by}`;
		case "approved":
			return `Approved${by}`;
		case "changesRequested":
			return `Changes requested${by}`;
		case "reviewRequested":
			return `Review requested${by}`;
		case "committed":
			return many ? `${entry.count} commits pushed${by}` : `Commit pushed${by}`;
		case "merged":
			return `Merged${by}`;
		case "closed":
			return `Closed${by}`;
	}
};

/** The branch and number, as the bell's second line shows them. */
const entryTarget = (entry: InboxEntry): string =>
	`${entry.sourceBranch} ${entry.unitSymbol}${entry.review}`;

/**
 * The id a summary notice carries: no entry answers to it, so its click only
 * brings the window forward.
 * @public exported for the test suite.
 */
export const summaryNoticeId = "summary";

/** Beyond this many in one poll, one summary stands in for the storm. */
const summaryAfter = 3;

/**
 * What the desktop shows for one poll's fresh entries: the loud ones, each
 * on its own, or a single summary when a catch-up files many at once.
 */
export const desktopNotices = (fresh: ReadonlyArray<InboxEntry>): Array<ShowNotificationParams> => {
	const loud = fresh.filter((entry) => inboxKindAttention[entry.kind] === "loud");
	if (loud.length <= summaryAfter) {
		return loud.map((entry) => ({
			id: entry.id,
			title: entryHeadline(entry),
			body: entry.snippet === null ? entryTarget(entry) : `${entryTarget(entry)}\n${entry.snippet}`,
		}));
	}
	return [
		{
			id: summaryNoticeId,
			title: `${loud.length} new notifications`,
			body: [...new Set(loud.map(entryTarget))].join(", "),
		},
	];
};

/**
 * File the poll's entries, keeping the list ordered by each entry's own
 * time. An id already filed is left exactly where it is, seen state and
 * all — a review bumping again must not resurface an old entry as new.
 * Returns new entries that survive retention, so replaying evicted history
 * cannot announce stale activity again.
 */
export const addInboxEntries = async (
	client: QueryClient,
	projectId: string,
	entries: Array<InboxEntry>,
): Promise<Array<InboxEntry>> => {
	if (entries.length === 0) return [];
	let filed: Array<InboxEntry> = [];
	await updateReviewState(client, projectId, (state) => {
		filed = [];
		const existing = state.inbox;
		const known = new Set(existing.map((entry) => entry.id));
		const fresh = entries.filter((entry) => !known.has(entry.id));
		if (fresh.length === 0) return state;
		const claimedIds = new Set(fresh.map((entry) => entry.id));
		const retainedExisting = existing.map((entry) =>
			entry.legacyId !== undefined && claimedIds.has(entry.legacyId)
				? { ...entry, legacyId: undefined }
				: entry,
		);
		const next = capInboxEntries(
			[...fresh, ...retainedExisting].sort((a, b) => Date.parse(b.at) - Date.parse(a.at)),
		);
		const retained = new Set(next);
		filed = fresh.filter((entry) => retained.has(entry));
		if (filed.length === 0) return state;
		return { ...state, inbox: next };
	});
	return filed;
};

/**
 * Mark the given entries seen; without ids, everything — which also
 * declares each represented review read up to its newest entry.
 */
export const markInboxSeen = async (
	client: QueryClient,
	projectId: string,
	ids?: ReadonlyArray<string>,
): Promise<void> => {
	await updateReviewState(client, projectId, (state) => {
		const { inbox } = state;
		const next =
			ids === undefined
				? reviewsSeenUpTo(
						state,
						inbox.map((entry) => [entry.review, entry.at] as const),
					)
				: state;
		const wanted = ids === undefined ? null : new Set(ids);
		if (!inbox.some((entry) => !entry.seen && (wanted === null || wanted.has(entry.id))))
			return next;
		return {
			...next,
			inbox: inbox.map((entry) =>
				!entry.seen && (wanted === null || wanted.has(entry.id)) ? { ...entry, seen: true } : entry,
			),
		};
	});
};

/** The entry a desktop notification was shown for, if it is still filed. */
export const findInboxEntry = async (
	client: QueryClient,
	projectId: string,
	id: string,
): Promise<InboxEntry | undefined> => {
	const { inbox: entries } = await client.fetchQuery(reviewStateQueryOptions(projectId));
	// Only migrated entries can answer to an old desktop notification ID.
	return entries.find((entry) => entry.id === id) ?? entries.find((entry) => entry.legacyId === id);
};

/** The inbox, newest first — a stable array identity between writes. */
export const useInboxEntries = (projectId: string, enabled: boolean): Array<InboxEntry> => {
	const { data } = useQuery({
		...reviewStateQueryOptions(projectId),
		enabled,
		select: (state) => state.inbox,
	});
	return enabled ? (data ?? emptyInbox) : emptyInbox;
};

const emptyInbox: Array<InboxEntry> = [];
