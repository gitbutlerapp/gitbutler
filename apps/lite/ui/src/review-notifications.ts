/**
 * @file Turning review activity into inbox entries.
 *
 * The decisions are pure and live in `review-activity.ts`; the hook here
 * feeds them the listing the app polls anyway. The unread dots stay the
 * record — the bell is the cross-review view of the same facts.
 */

import {
	activityItems,
	attentionOf,
	itemMentions,
	observeReviews,
	seenLedger,
	type ActivityLedger,
	type ReviewActivityItem,
	type ReviewChange,
} from "#ui/review-activity.ts";
import {
	currentForgeLoginQueryOptions,
	forgeInfoOptions,
	headInfoQueryOptions,
	listReviewCommentsQueryOptions,
	listReviewSubmissionsQueryOptions,
	listReviewTimelineEventsQueryOptions,
	listReviewsQueryOptions,
} from "#ui/api/queries.ts";
import { addInboxEntries, type InboxEntry, type InboxKind } from "#ui/review-inbox.ts";
import { readSeenMarks, usePrNotificationsLevel } from "#ui/review-seen.ts";
import { selfMergedNumbers } from "#ui/api/mutations.ts";
import type { ForgeReview, RefInfo } from "@gitbutler/but-sdk";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useEffectEvent, useRef } from "react";

/** @public shared with the bell, whose entry clicks jump the same way. */
export const appliedRefsByName = (headInfo: RefInfo): Map<string, Array<number>> =>
	new Map(
		headInfo.stacks.flatMap((stack) =>
			stack.segments.flatMap((segment) =>
				segment.refName
					? [[segment.refName.displayName, segment.refName.fullNameBytes] as const]
					: [],
			),
		),
	);

/** The kind an item files under; mentions outrank the item's own shape. */
const inboxKindOf = (item: ReviewActivityItem, login: string | null): InboxKind => {
	if (itemMentions(item, login)) return "mention";
	switch (item.kind) {
		case "comment":
			return "comment";
		case "verdict":
			// A "commented" verdict is the note a comment batch arrives with.
			if (item.state === "approved") return "approved";
			if (item.state === "changesRequested") return "changesRequested";
			return "comment";
		case "reviewRequested":
			return "reviewRequested";
		case "committed":
			return "committed";
	}
};

const firstLine = (body: string | null): string | null => {
	const line = body?.split("\n").find((candidate) => candidate.trim() !== "");
	return line === undefined ? null : line.trim().slice(0, 140);
};

/** One coalesced entry: a kind's items on one review in one poll. */
const entryOf = (
	review: ForgeReview,
	kind: InboxKind,
	bucket: Array<ReviewActivityItem>,
): InboxEntry => {
	const newest = bucket.reduce<ReviewActivityItem | null>(
		(best, item) => (best === null || item.atMs > best.atMs ? item : best),
		null,
	);
	const at =
		newest === null
			? (review.modifiedAt ?? new Date().toISOString())
			: new Date(newest.atMs).toISOString();
	return {
		id: `${review.number}:${kind}:${at}`,
		kind,
		review: review.number,
		reviewTitle: review.title,
		unitSymbol: review.unitSymbol,
		sourceBranch: review.sourceBranch,
		htmlUrl: review.htmlUrl,
		author: newest?.author ?? null,
		count: Math.max(bucket.length, 1),
		snippet:
			newest !== null && (newest.kind === "comment" || newest.kind === "verdict")
				? firstLine(newest.body)
				: null,
		at,
		seen: false,
	};
};

/**
 * Watch the review listing and file activity into the inbox. The first
 * listing observed is the baseline — nothing older is filed; the dots carry
 * that. Applied-branch reviews file everything short of silent, the rest
 * only mentions, and a kind's items on one review coalesce into one entry.
 */
export const useReviewActivityInbox = (projectId: string): void => {
	const client = useQueryClient();
	const level = usePrNotificationsLevel();

	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const enabled = level === "loud" && !!forgeInfo?.capabilities.prService;
	const { data: reviews } = useQuery({
		...listReviewsQueryOptions({ projectId, cacheConfig: "noCache" }),
		enabled,
	});
	const { data: appliedRefs } = useQuery({
		...headInfoQueryOptions(projectId),
		select: appliedRefsByName,
		enabled,
	});
	const { data: selfLogin, isPending: loginPending } = useQuery({
		...currentForgeLoginQueryOptions(projectId),
		enabled,
	});

	const ledger = useRef<ActivityLedger | null>(null);

	const entriesOf = useEffectEvent(async (change: ReviewChange, applied: boolean) => {
		const login = selfLogin ?? null;
		if (change.settledNow !== null) {
			if (!applied || selfMergedNumbers(client, projectId).has(change.review.number)) return [];
			return [entryOf(change.review, change.settledNow, [])];
		}
		// Without conversation listings there is nothing to classify — the
		// bump stays a quiet dot rather than guessing at loudness.
		if (forgeInfo?.capabilities.reviewComments === false) return [];
		const reviewId = change.review.number;
		// The client caches at staleTime Infinity; without an explicit 0 these
		// would silently answer from cache and re-classify old items as new.
		const [comments, submissions, events] = await Promise.all([
			client.fetchQuery({
				...listReviewCommentsQueryOptions({ projectId, reviewId }),
				staleTime: 0,
			}),
			client.fetchQuery({
				...listReviewSubmissionsQueryOptions({ projectId, reviewId }),
				staleTime: 0,
			}),
			// Mentions live in comment and verdict text, so a review outside
			// the workspace does not need the timeline.
			applied
				? client.fetchQuery({
						...listReviewTimelineEventsQueryOptions({ projectId, reviewId }),
						staleTime: 0,
					})
				: Promise.resolve([]),
		]);
		// Outside the workspace only a mention is the user's business; on an
		// applied branch anything short of silent lands in the inbox — the
		// bell holds quiet facts like pushes that a toast never carried.
		const items = activityItems(comments, submissions, events, change.sinceMs).filter((item) => {
			const attention = attentionOf(item, login);
			if (attention === "silent") return false;
			if (!applied && !itemMentions(item, login)) return false;
			// A request naming someone else, or a dismissed verdict, is
			// bookkeeping rather than a message.
			if (item.kind === "reviewRequested" && attention !== "loud") return false;
			if (item.kind === "verdict" && item.state === "dismissed") return false;
			return true;
		});
		const buckets = new Map<InboxKind, Array<ReviewActivityItem>>();
		for (const item of items) {
			const kind = inboxKindOf(item, login);
			const bucket = buckets.get(kind);
			if (bucket) bucket.push(item);
			else buckets.set(kind, [item]);
		}
		return [...buckets].map(([kind, bucket]) => entryOf(change.review, kind, bucket));
	});

	const observe = useEffectEvent(async (listing: Array<ForgeReview>) => {
		if (!appliedRefs) return;
		if (ledger.current === null) {
			// Seeded from what has actually been seen, so activity that landed
			// while the app was closed still speaks up.
			ledger.current = seenLedger(listing, readSeenMarks(projectId));
		}
		const { changed, next } = observeReviews(ledger.current, listing);
		ledger.current = next;
		if (changed.length === 0) return;

		// Classified in parallel: one slow review must not hold back the rest.
		const entries = (
			await Promise.all(
				changed.map(async (change) => {
					try {
						return await entriesOf(change, appliedRefs.has(change.review.sourceBranch));
					} catch {
						// A flaky forge read costs one entry; the unread dot still shows.
						return [];
					}
				}),
			)
		).flat();
		addInboxEntries(projectId, entries);
	});

	// A disabled detector forgets its baseline, so re-enabling starts from
	// the then-current listing instead of replaying the interim as news.
	useEffect(() => {
		if (!enabled) ledger.current = null;
	}, [enabled, projectId]);

	// `appliedRefs` in the deps takes the baseline as soon as both the
	// listing and the applied set exist, whichever resolves last. The login
	// holds it too: classifying before it loads would consume a bump with
	// mention detection blind, and the ledger never re-examines a change.
	// A login query *error* proceeds with no login rather than going deaf.
	useEffect(() => {
		if (enabled && reviews && !loginPending) void observe(reviews.reviews);
	}, [enabled, reviews, appliedRefs, loginPending]);
};
