import {
	currentForgeLoginQueryOptions,
	forgeInfoOptions,
	guiSettingsQueryOptions,
	listReviewsQueryOptions,
} from "#ui/api/queries.ts";
import { defaultSettings } from "#ui/settings.ts";
import {
	useQuery,
	useQueryClient,
	useSuspenseQuery,
	type QueryClient,
} from "@tanstack/react-query";
import {
	reviewStateQueryOptions,
	updateReviewState,
	unseenCap,
	type ReviewState,
	type SeenMarks,
	type UnseenEntries,
} from "#ui/review-state.ts";
import { createContext, useEffect, useEffectEvent, useState } from "react";

/**
 * The PR-notifications dial: loud files activity into the bell, quiet
 * tracks what was seen without showing it, off hides the tracking UI entirely.
 */
export const usePrNotificationsLevel = (): "loud" | "quiet" | "off" => {
	const { data: level } = useQuery({
		...guiSettingsQueryOptions,
		select: (settings) => settings.prNotifications ?? defaultSettings.prNotifications,
	});
	return level ?? defaultSettings.prNotifications;
};

/** Whether loud activity also reaches the desktop while the window is unfocused. */
export const useDesktopNotifications = (): boolean => {
	const { data: enabled } = useQuery({
		...guiSettingsQueryOptions,
		select: (settings) => settings.desktopNotifications ?? defaultSettings.desktopNotifications,
	});
	return enabled ?? defaultSettings.desktopNotifications;
};

/**
 * What the PR view is showing, so the dwell knows which items above the old
 * watermark were on offer, and which the reader already looked at. In memory
 * only: it is re-registered every visit, and losing the pending set merely
 * shows a marker once more.
 */
const shownItems = new Map<string, Map<string, Array<{ key: string; atMs: number }>>>();
const pendingSeen = new Map<string, Set<string>>();
const reviewSlot = (projectId: string, reviewNumber: number) => `${projectId}:${reviewNumber}`;

/** Whether one item is recorded as skipped — unread below the watermark. */
export const isItemSkipped = (state: ReviewState, reviewNumber: number, key: string): boolean =>
	(state.unseen[reviewNumber] ?? []).some(([k]) => k === key);

/**
 * Tell the store which unread-eligible items one surface of the review is
 * showing. Sources register independently so a surface that renders only
 * part of the timeline does not erase another's registration.
 */
export const registerReviewItems = (
	projectId: string,
	reviewNumber: number,
	source: string,
	items: Array<{ key: string; atMs: number }>,
): void => {
	const slot = reviewSlot(projectId, reviewNumber);
	const sources = shownItems.get(slot) ?? new Map<string, Array<{ key: string; atMs: number }>>();
	sources.set(source, items);
	shownItems.set(slot, sources);
};

/**
 * A surface unmounted: its registration must not linger as "on offer", and
 * the map must not grow with every review ever visited.
 */
export const unregisterReviewItems = (
	projectId: string,
	reviewNumber: number,
	source: string,
): void => {
	const slot = reviewSlot(projectId, reviewNumber);
	const sources = shownItems.get(slot);
	sources?.delete(source);
	if (sources?.size === 0) shownItems.delete(slot);
};

/**
 * One item was actually looked at. Before the dwell has advanced the
 * watermark it pre-empts the skip; after, it clears the skip.
 */
export const markItemSeen = async (
	client: QueryClient,
	projectId: string,
	reviewNumber: number,
	key: string,
): Promise<void> => {
	const cached = client.getQueryData(reviewStateQueryOptions(projectId).queryKey);
	// Register pre-dwell reads synchronously, before the storage write can yield.
	if (cached === undefined || !isItemSkipped(cached, reviewNumber, key)) {
		const slot = reviewSlot(projectId, reviewNumber);
		const pending = pendingSeen.get(slot) ?? new Set<string>();
		pending.add(key);
		pendingSeen.set(slot, pending);
	}
	await updateReviewState(client, projectId, (state) => {
		const skipped = state.unseen[reviewNumber];
		if (!skipped?.some(([k]) => k === key)) return state;
		const unseen = { ...state.unseen };
		const kept = skipped.filter(([k]) => k !== key);
		if (kept.length > 0) unseen[reviewNumber] = kept;
		else delete unseen[reviewNumber];
		return { ...state, unseen };
	});
};

/** Whether activity at `modifiedAt` is newer than the watermark. */
const pastMark = (modifiedAt: string | null, seen: string | undefined): boolean =>
	modifiedAt !== null && seen !== undefined && Date.parse(modifiedAt) > Date.parse(seen);

/**
 * Whether one review has unread activity — a boolean, so a watermark moving
 * on another review leaves this subscriber alone.
 */
const useReviewUnread = (
	projectId: string,
	review: { number: number; modifiedAt: string | null },
	enabled: boolean,
): boolean => {
	const { number, modifiedAt } = review;
	const { data: unread = false } = useQuery({
		...reviewStateQueryOptions(projectId),
		enabled,
		select: (state) =>
			pastMark(modifiedAt, state.marks[number]) || (state.unseen[number]?.length ?? 0) > 0,
	});
	return enabled && unread;
};

/**
 * Stamp reviews seen when first listed and prune marks for delisted ones.
 * An absent mark reads as seen, so the stamp is what makes only activity
 * after first sight count as unread. Mounted once per project surface.
 */
export const useStampReviewsSeen = (projectId: string): void => {
	const client = useQueryClient();
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const level = usePrNotificationsLevel();
	const enabled = !!forgeInfo?.capabilities.prService && level !== "off";
	const { data: listed } = useQuery({
		...listReviewsQueryOptions({ projectId, cacheConfig: "noCache" }),
		enabled,
		// A plain array keeps this subscriber's data small.
		select: (reviews) =>
			reviews.map((review) => ({ number: review.number, modifiedAt: review.modifiedAt })),
	});

	const reconcile = useEffectEvent((reviews: NonNullable<typeof listed>) => {
		void updateReviewState(client, projectId, (state) => {
			const { marks, unseen } = state;
			const next: SeenMarks = {};
			let stamped = false;
			for (const { number, modifiedAt } of reviews) {
				const seen = marks[number];
				if (seen !== undefined) {
					next[number] = seen;
				} else if (modifiedAt !== null) {
					next[number] = modifiedAt;
					stamped = true;
				}
			}
			// A mark whose review is gone from the listing is dead weight.
			const pruned = Object.keys(next).length !== Object.keys(marks).length;
			const keptUnseen: UnseenEntries = {};
			for (const [number, entries] of Object.entries(unseen))
				if (Number(number) in next) keptUnseen[Number(number)] = entries;
			return stamped || pruned || Object.keys(keptUnseen).length !== Object.keys(unseen).length
				? { ...state, marks: next, unseen: keptUnseen }
				: state;
		});
	});

	useEffect(() => {
		if (listed) reconcile(listed);
	}, [listed]);
};

/** What had been seen when the PR view opened; nothing is new outside one. */
type SeenOnArrival = {
	sinceMs: number;
	selfLogin: string | null;
	projectId: string;
	reviewNumber: number;
};

export const SeenOnArrivalContext = createContext<SeenOnArrival>({
	sinceMs: Infinity,
	selfLogin: null,
	projectId: "",
	reviewNumber: 0,
});

/**
 * The watermark as it stood when the view mounted — a snapshot, because the
 * dwell advances the live mark right after arrival and the "New" badges
 * must not vanish under the reader. The next visit starts clean.
 */
export const useSeenOnArrival = (projectId: string, reviewNumber: number): SeenOnArrival => {
	const { data: mark } = useSuspenseQuery({
		...reviewStateQueryOptions(projectId),
		select: (state) => state.marks[reviewNumber] ?? null,
	});
	const { data: selfLogin } = useQuery(currentForgeLoginQueryOptions(projectId));
	const level = usePrNotificationsLevel();
	const [sinceMs] = useState(() => {
		const ms = mark === null ? Number.NaN : Date.parse(mark);
		// No watermark means the review was never tracked; nothing is new.
		return Number.isNaN(ms) ? Infinity : ms;
	});
	// Off means off: no markers, and no seen-state writes from the observer.
	if (level === "off") return { sinceMs: Infinity, selfLogin: null, projectId, reviewNumber: 0 };
	return { sinceMs, selfLogin: selfLogin ?? null, projectId, reviewNumber };
};

/**
 * Advance the watermark, recording the registered items above it that the
 * reader has not looked at as skips — their markers survive the advance and
 * the dot stays lit until each is seen.
 *
 * @public exported for the store transitions in the test suite.
 */
export const markReviewSeen = async (
	client: QueryClient,
	projectId: string,
	number: number,
	modifiedAt: string,
): Promise<void> => {
	const slot = reviewSlot(projectId, number);
	const pending = pendingSeen.get(slot) ?? new Set<string>();
	pendingSeen.delete(slot);
	const offered = [...(shownItems.get(slot)?.values() ?? [])].flat();
	await updateReviewState(client, projectId, (state) => {
		const floor = state.marks[number];
		const floorMs = floor === undefined ? Infinity : Date.parse(floor);
		const skipped = new Map((state.unseen[number] ?? []).map(([k, at]) => [k, at]));
		for (const item of offered) {
			if (item.atMs > floorMs && !pending.has(item.key) && !skipped.has(item.key))
				skipped.set(item.key, new Date(item.atMs).toISOString());
		}
		const entries = [...skipped]
			.sort(([, a], [, b]) => Date.parse(a) - Date.parse(b))
			// Beyond the cap the oldest are dropped: reading as seen is the safe
			// failure, and a skip that old was never getting read.
			.slice(-unseenCap);
		const nextUnseen = { ...state.unseen };
		if (entries.length > 0) nextUnseen[number] = entries;
		else delete nextUnseen[number];
		return {
			...state,
			unseen: nextUnseen,
			marks: {
				...state.marks,
				[number]:
					floor !== undefined && Date.parse(floor) > Date.parse(modifiedAt) ? floor : modifiedAt,
			},
		};
	});
};

/**
 * Declare activity read up to `latest`, one or more stamps per review:
 * each watermark advances to the newest given — never backwards, a dwell
 * may already have moved it further — and the review's skips at or before
 * that newest stamp are dropped. A skip after it is still unread.
 */
export const reviewsSeenUpTo = (
	state: ReviewState,
	latest: Iterable<readonly [number, string]>,
): ReviewState => {
	const marks = { ...state.marks };
	const unseen = { ...state.unseen };
	let marksChanged = false;
	let unseenChanged = false;
	// Order-free: the advance is monotonic and the skip filters compose, so
	// repeated stamps for a review settle on the newest whichever comes first.
	for (const [number, at] of latest) {
		const atMs = Date.parse(at);
		const seen = marks[number];
		if (seen === undefined || atMs > Date.parse(seen)) {
			marks[number] = at;
			marksChanged = true;
		}
		const skipped = unseen[number];
		if (skipped === undefined) continue;
		const kept = skipped.filter(([, skippedAt]) => Date.parse(skippedAt) > atMs);
		if (kept.length === skipped.length) continue;
		if (kept.length > 0) unseen[number] = kept;
		else delete unseen[number];
		unseenChanged = true;
	}
	return marksChanged || unseenChanged ? { ...state, marks, unseen } : state;
};

/** A beat, so flicking past a review does not eat its unread state. */
const dwellMs = 1000;

/**
 * Advance the review's watermark while its PR tab is on screen: a dwell
 * after mount or new activity, re-armed when the window regains focus so
 * viewing an unfocused window does not count as seeing.
 */
export const useMarkReviewSeenOnView = (
	projectId: string,
	review: { number: number; modifiedAt: string | null },
	enabled: boolean,
): void => {
	const client = useQueryClient();
	const { number, modifiedAt } = review;
	// Nothing unread means nothing to write, on remount or with tracking off.
	const behind = useReviewUnread(projectId, review, enabled);

	const mark = useEffectEvent(() => {
		if (modifiedAt === null || !behind || !document.hasFocus()) return;
		void markReviewSeen(client, projectId, number, modifiedAt);
	});

	// `behind` in the deps re-arms the dwell once the watermarks load.
	useEffect(() => {
		if (modifiedAt === null || !behind) return;
		let timer: number | undefined;
		const arm = () => {
			clearTimeout(timer);
			timer = window.setTimeout(mark, dwellMs);
		};
		window.addEventListener("focus", arm);
		if (document.hasFocus()) arm();
		return () => {
			clearTimeout(timer);
			window.removeEventListener("focus", arm);
		};
	}, [projectId, number, modifiedAt, behind]);
};
