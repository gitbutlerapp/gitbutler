/**
 * @file Marking what arrived since the user last looked.
 *
 * The PR view provides `SeenOnArrivalContext` from `review-seen.ts`; any
 * timeline entry newer than that snapshot — or recorded as skipped on an
 * earlier visit — wears this marker. The marker is also the seer: once it
 * has sat in the focused viewport for a beat, it reports its item seen, and
 * only then does the next visit consider the item read.
 */

import { classes } from "#ui/components/classes.ts";
import {
	isItemSkipped,
	markItemSeen,
	registerReviewItems,
	SeenOnArrivalContext,
	unregisterReviewItems,
} from "#ui/review-seen.ts";
import type { ForgeReviewUser } from "@gitbutler/but-sdk";
import { useContext, useEffect, useRef, useState, type FC } from "react";
import styles from "./review-arrival.module.css";

/** In view this long, focused, before an item counts as looked at. */
const seenBeatMs = 1000;

/**
 * Registers a surface's unread-eligible items — what the dwell may record
 * as skipped — keyed the way their markers key themselves. Renders nothing.
 */
export const RegisterFreshItems: FC<{
	source: string;
	items: Array<{ key: string; atMs: number }>;
}> = ({ source, items }) => {
	const { projectId, reviewNumber } = useContext(SeenOnArrivalContext);
	useEffect(() => {
		registerReviewItems(projectId, reviewNumber, source, items);
		return () => unregisterReviewItems(projectId, reviewNumber, source);
	}, [projectId, reviewNumber, source, items]);
	return null;
};

/** "New" beside a timeline entry that landed after the reader last looked. */
export const FreshBadge: FC<{
	timestamp: number | null;
	author?: ForgeReviewUser | null;
	/** The item's stable key; without one the marker cannot be seen away. */
	itemKey?: string;
}> = ({ timestamp, author, itemKey }) => {
	const { sinceMs, selfLogin, projectId, reviewNumber } = useContext(SeenOnArrivalContext);
	const ref = useRef<HTMLSpanElement | null>(null);
	// Decided at mount and held: store writes must not pull the marker out
	// from under the reader mid-visit. The next visit re-decides.
	const [show] = useState(() => {
		// The reader's own actions post-date arrival by definition; not news.
		const own =
			author != null &&
			selfLogin !== null &&
			author.login.toLowerCase() === selfLogin.toLowerCase();
		if (own || timestamp === null) return false;
		return (
			timestamp > sinceMs ||
			(itemKey !== undefined && isItemSkipped(projectId, reviewNumber, itemKey))
		);
	});

	useEffect(() => {
		const el = ref.current;
		if (!show || itemKey === undefined || el === null) return;
		let timer: number | undefined;
		let inView = false;
		const arm = () => {
			clearTimeout(timer);
			timer = window.setTimeout(() => {
				if (document.hasFocus()) markItemSeen(projectId, reviewNumber, itemKey);
			}, seenBeatMs);
		};
		const observer = new IntersectionObserver(([entry]) => {
			inView = entry?.isIntersecting ?? false;
			if (inView) arm();
			else clearTimeout(timer);
		});
		observer.observe(el);
		// An unfocused beat writes nothing; regaining focus re-arms it.
		const onFocus = () => {
			if (inView) arm();
		};
		window.addEventListener("focus", onFocus);
		return () => {
			clearTimeout(timer);
			observer.disconnect();
			window.removeEventListener("focus", onFocus);
		};
	}, [show, itemKey, projectId, reviewNumber]);

	if (!show) return null;
	return (
		<span className={classes("text-11", "text-semibold", styles.fresh)} ref={ref}>
			New
		</span>
	);
};
