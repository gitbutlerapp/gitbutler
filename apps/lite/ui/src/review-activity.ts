/**
 * @file Deciding which review activity deserves the user's attention.
 *
 * The axis: is a human waiting on you? Everything here is pure so the test
 * suite can drive it with fabricated events; `review-notifications.ts` feeds
 * it live listings and turns its verdicts into toasts.
 */

import type {
	ForgeReview,
	ForgeReviewComment,
	ForgeReviewSubmission,
	ForgeReviewSubmissionState,
	ForgeReviewTimelineEvent,
} from "@gitbutler/but-sdk";

/**
 * One thing that happened on a review, reduced to what classification needs.
 *
 * @public exported for the fabricated events in the test suite.
 */
export type ReviewActivityItem =
	| { kind: "comment"; author: string | null; body: string; atMs: number }
	| {
			kind: "verdict";
			author: string | null;
			state: ForgeReviewSubmissionState;
			body: string | null;
			atMs: number;
	  }
	| {
			kind: "reviewRequested";
			author: string | null;
			requestedReviewer: string | null;
			atMs: number;
	  }
	| { kind: "committed"; author: string | null; atMs: number };

type Attention = "loud" | "quiet" | "silent";

/** Forge logins compare case-insensitively. */
const sameLogin = (a: string | null, b: string | null): boolean =>
	a !== null && b !== null && a.toLowerCase() === b.toLowerCase();

/**
 * Whether the item's text @-mentions the login. Login characters may follow
 * an ordinary word boundary (`@alice-b` is not `@alice`), so the ends are
 * checked explicitly.
 */
export const itemMentions = (item: ReviewActivityItem, login: string | null): boolean => {
	if (login === null) return false;
	const body = item.kind === "comment" || item.kind === "verdict" ? item.body : null;
	if (body === null) return false;
	const escaped = login.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
	return new RegExp(`(?<![\\w-])@${escaped}(?![\\w-])`, "i").test(body);
};

/**
 * Loud means a human is waiting on the user. Own actions are silent, a
 * mention always waits, a dismissed verdict is bookkeeping rather than a
 * message, and a review request only matters when it names the user.
 */
export const attentionOf = (item: ReviewActivityItem, selfLogin: string | null): Attention => {
	if (sameLogin(item.author, selfLogin)) return "silent";
	if (itemMentions(item, selfLogin)) return "loud";
	switch (item.kind) {
		case "comment":
			return "loud";
		case "verdict":
			return item.state === "dismissed" ? "quiet" : "loud";
		case "reviewRequested":
			return sameLogin(item.requestedReviewer, selfLogin) ? "loud" : "quiet";
		case "committed":
			return "quiet";
	}
};

/** What the detector remembers about a review between polls. */
export type ReviewObservation = {
	modifiedAtMs: number;
	settled: boolean;
};

export type ActivityLedger = ReadonlyMap<number, ReviewObservation>;

export type ReviewChange = {
	review: ForgeReview;
	/** Activity strictly after this counts as new. */
	sinceMs: number;
	/** The settling observed on this poll, if the review just settled. */
	settledNow: "merged" | "closed" | null;
};

const parseMs = (value: string | null): number => {
	if (value === null) return 0;
	const ms = Date.parse(value);
	return Number.isNaN(ms) ? 0 : ms;
};

const observationOf = (review: ForgeReview): ReviewObservation => ({
	modifiedAtMs: parseMs(review.modifiedAt),
	settled: review.mergedAt !== null || review.closedAt !== null,
});

/**
 * Fold one poll's listing into the ledger and report which reviews moved.
 * First sight is recorded but never reported — at app start everything is
 * first seen, and replaying history would be a storm. Unlisted reviews drop
 * out of the ledger.
 */
export const observeReviews = (
	ledger: ActivityLedger,
	reviews: Array<ForgeReview>,
): { changed: Array<ReviewChange>; next: ActivityLedger } => {
	const changed: Array<ReviewChange> = [];
	const next = new Map<number, ReviewObservation>();
	for (const review of reviews) {
		const now = observationOf(review);
		next.set(review.number, now);
		const prev = ledger.get(review.number);
		if (prev === undefined || now.modifiedAtMs <= prev.modifiedAtMs) continue;
		changed.push({
			review,
			sinceMs: prev.modifiedAtMs,
			settledNow:
				!prev.settled && now.settled ? (review.mergedAt !== null ? "merged" : "closed") : null,
		});
	}
	return { changed, next };
};

/**
 * The ledger to start from: what the user has already seen, so activity that
 * arrived while the app was closed still announces itself once. A review with
 * no watermark baselines at its current state — first sight is never news.
 */
export const seenLedger = (
	reviews: Array<ForgeReview>,
	seen: Record<number, string>,
): ActivityLedger =>
	new Map(
		reviews.map((review) => {
			const mark = seen[review.number];
			return [
				review.number,
				mark === undefined
					? observationOf(review)
					: { ...observationOf(review), modifiedAtMs: parseMs(mark) },
			];
		}),
	);

/**
 * Reduce the fetched conversation to items strictly after `sinceMs`. Undated
 * entries are left out — they would re-report on every poll.
 */
export const activityItems = (
	comments: Array<ForgeReviewComment>,
	submissions: Array<ForgeReviewSubmission>,
	events: Array<ForgeReviewTimelineEvent>,
	sinceMs: number,
): Array<ReviewActivityItem> => {
	const items: Array<ReviewActivityItem> = [];
	for (const comment of comments) {
		const atMs = parseMs(comment.createdAt);
		if (atMs > sinceMs) {
			items.push({
				kind: "comment",
				author: comment.author?.login ?? null,
				body: comment.body,
				atMs,
			});
		}
	}
	for (const submission of submissions) {
		const atMs = parseMs(submission.submittedAt);
		if (atMs > sinceMs) {
			items.push({
				kind: "verdict",
				author: submission.author?.login ?? null,
				state: submission.state,
				body: submission.body,
				atMs,
			});
		}
	}
	for (const event of events) {
		const atMs = parseMs(event.createdAt);
		if (atMs <= sinceMs) continue;
		const author = event.actor?.login ?? null;
		if (event.kind === "reviewRequested") {
			items.push({
				kind: "reviewRequested",
				author,
				requestedReviewer: event.requestedReviewer?.login ?? null,
				atMs,
			});
		} else {
			items.push({ kind: "committed", author, atMs });
		}
	}
	return items;
};
