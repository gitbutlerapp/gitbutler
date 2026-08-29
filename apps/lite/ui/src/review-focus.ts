/**
 * @file The comment a notification asked to show.
 *
 * A toast names one comment; clicking View selects the review and leaves the
 * request here. The conversation picks it up and scrolls, which is what makes
 * View worth pressing when that review is already on screen.
 */

import { useSyncExternalStore } from "react";

let requested: { reviewId: number; commentId: number } | null = null;
const listeners = new Set<() => void>();

const notify = () => {
	for (const listener of listeners) listener();
};

const subscribe = (listener: () => void): (() => void) => {
	listeners.add(listener);
	return () => listeners.delete(listener);
};

export const requestReviewFocus = (reviewId: number, commentId: number): void => {
	requested = { reviewId, commentId };
	notify();
};

/** Consumed once: the conversation clears it as soon as it has scrolled. */
export const clearReviewFocus = (): void => {
	requested = null;
	notify();
};

/** The comment this review is being asked to show, if any. */
export const useRequestedComment = (reviewId: number): number | null =>
	useSyncExternalStore(subscribe, () =>
		requested?.reviewId === reviewId ? requested.commentId : null,
	);
