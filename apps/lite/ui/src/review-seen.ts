/**
 * @file Which review activity the user has seen.
 *
 * One watermark per review — "everything up to this `modifiedAt` has been
 * seen" — in local storage: disposable, per-machine, and synchronous, so a
 * plain external store rather than a query. Every hook returns a primitive,
 * so React skips the render when a write leaves it unchanged.
 */

import {
	currentForgeLoginQueryOptions,
	forgeInfoOptions,
	guiSettingsQueryOptions,
	listReviewsQueryOptions,
} from "#ui/api/queries.ts";
import { defaultSettings } from "#ui/settings.ts";
import { useQuery } from "@tanstack/react-query";
import { createContext, useEffect, useEffectEvent, useState, useSyncExternalStore } from "react";

/**
 * The PR-notifications dial: loud files activity into the bell, quiet
 * keeps only the unread dots, off hides the tracking UI entirely.
 */
export const usePrNotificationsLevel = (): "loud" | "quiet" | "off" => {
	const { data: level } = useQuery({
		...guiSettingsQueryOptions,
		select: (settings) => settings.prNotifications ?? defaultSettings.prNotifications,
	});
	return level ?? defaultSettings.prNotifications;
};

/** Watermark by review number, as the ISO stamps the forge reports. */
type SeenMarks = Record<number, string>;

const storageKey = (projectId: string) => `pr_activity_seen:v1:${projectId}`;

const listeners = new Set<() => void>();
// Snapshot reads are pure in-memory lookups — a write fans out to one per
// branch row. Storage is touched only on a miss, a write, or invalidation.
let cached: { key: string; marks: SeenMarks } | null = null;

// Storage can throw — disabled, partitioned, or over quota — and a throw
// here would crash every subscriber's render. Tracking degrades instead.
const storageGet = (key: string): string | null => {
	try {
		return localStorage.getItem(key);
	} catch {
		return null;
	}
};
const storageSet = (key: string, value: string): void => {
	try {
		localStorage.setItem(key, value);
	} catch {
		// The in-memory copy still serves this session.
	}
};

/**
 * Only valid watermarks survive the parse: a foreign entry or unparseable
 * stamp would baseline the detector at epoch and replay history as news.
 */
const parseMarks = (raw: string | null): SeenMarks => {
	if (raw === null) return {};
	try {
		const stored: unknown = JSON.parse(raw);
		if (typeof stored !== "object" || stored === null || Array.isArray(stored)) return {};
		const marks: SeenMarks = {};
		for (const [number, seen] of Object.entries(stored)) {
			if (typeof seen === "string" && /^\d+$/.test(number) && !Number.isNaN(Date.parse(seen)))
				marks[Number(number)] = seen;
		}
		return marks;
	} catch {
		return {};
	}
};

const readMarks = (projectId: string): SeenMarks => {
	const key = storageKey(projectId);
	if (cached?.key === key) return cached.marks;
	const marks = parseMarks(storageGet(key));
	cached = { key, marks };
	return marks;
};

/**
 * Items the reader jumped over: below the watermark yet still unread. The
 * complement is stored on purpose — a list of *read* items would grow with
 * everything ever read once one stubborn item pinned the mark, while skipped
 * items stay few by nature and dropping one merely reads as seen.
 */
type UnseenEntries = Record<number, Array<[key: string, at: string]>>;

const unseenStorageKey = (projectId: string) => `pr_activity_unseen:v1:${projectId}`;
let cachedUnseen: { key: string; entries: UnseenEntries } | null = null;

/** Skipped items per review, oldest first; dropping the overflow reads as seen. */
const unseenCap = 50;

const parseUnseen = (raw: string | null): UnseenEntries => {
	if (raw === null) return {};
	try {
		const stored: unknown = JSON.parse(raw);
		if (typeof stored !== "object" || stored === null || Array.isArray(stored)) return {};
		const entries: UnseenEntries = {};
		for (const [number, list] of Object.entries(stored)) {
			if (!/^\d+$/.test(number) || !Array.isArray(list)) continue;
			const kept = list.filter(
				(entry): entry is [string, string] =>
					Array.isArray(entry) &&
					typeof entry[0] === "string" &&
					typeof entry[1] === "string" &&
					!Number.isNaN(Date.parse(entry[1])),
			);
			// The newest survive the cap, as the writer keeps them.
			if (kept.length > 0) entries[Number(number)] = kept.slice(-unseenCap);
		}
		return entries;
	} catch {
		return {};
	}
};

const readUnseen = (projectId: string): UnseenEntries => {
	const key = unseenStorageKey(projectId);
	if (cachedUnseen?.key === key) return cachedUnseen.entries;
	const entries = parseUnseen(storageGet(key));
	cachedUnseen = { key, entries };
	return entries;
};

const setUnseen = (projectId: string, entries: UnseenEntries): void => {
	storageSet(unseenStorageKey(projectId), JSON.stringify(entries));
	cachedUnseen = { key: unseenStorageKey(projectId), entries };
};

const writeUnseen = (projectId: string, entries: UnseenEntries): void => {
	setUnseen(projectId, entries);
	notify();
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
export const isItemSkipped = (projectId: string, reviewNumber: number, key: string): boolean =>
	(readUnseen(projectId)[reviewNumber] ?? []).some(([k]) => k === key);

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
export const markItemSeen = (projectId: string, reviewNumber: number, key: string): void => {
	const entries = readUnseen(projectId);
	const skipped = entries[reviewNumber];
	if (skipped?.some(([k]) => k === key)) {
		const next = { ...entries };
		const kept = skipped.filter(([k]) => k !== key);
		if (kept.length > 0) next[reviewNumber] = kept;
		else delete next[reviewNumber];
		writeUnseen(projectId, next);
		return;
	}
	const slot = reviewSlot(projectId, reviewNumber);
	const pending = pendingSeen.get(slot) ?? new Set<string>();
	pending.add(key);
	pendingSeen.set(slot, pending);
};

const notify = (): void => {
	for (const listener of listeners) listener();
};

const writeMarks = (projectId: string, marks: SeenMarks): void => {
	storageSet(storageKey(projectId), JSON.stringify(marks));
	cached = { key: storageKey(projectId), marks };
	notify();
};

let watchingStorage = false;

/** A write from another window arrives as a storage event. */
const onStorage = (event: StorageEvent): void => {
	// A null key is a wholesale clear; anything else outside this feature's
	// keys is someone else's business.
	if (event.key !== null && !event.key.startsWith("pr_activity_")) return;
	cached = null;
	cachedUnseen = null;
	notify();
};

const subscribeMarks = (listener: () => void): (() => void) => {
	// On first use, so merely importing this module listens to nothing.
	if (!watchingStorage) {
		watchingStorage = true;
		window.addEventListener("storage", onStorage);
	}
	// A first subscriber is a fresh surface: re-read whatever storage holds.
	if (listeners.size === 0) {
		cached = null;
		cachedUnseen = null;
	}
	listeners.add(listener);
	return () => listeners.delete(listener);
};

/** A disabled hook stays out of the fan-out entirely. */
const subscribeNothing = (): (() => void) => () => {};

/** The stored watermarks, for the activity detector's startup baseline. */
export const readSeenMarks = (projectId: string): Record<number, string> => readMarks(projectId);

/** Whether activity at `modifiedAt` is newer than the watermark. */
const pastMark = (modifiedAt: string | null, seen: string | undefined): boolean =>
	modifiedAt !== null && seen !== undefined && Date.parse(modifiedAt) > Date.parse(seen);

/** Unread: the review moved past the watermark, or skipped items remain. */
const isUnread = (
	projectId: string,
	number: number,
	modifiedAt: string | null,
	marks: SeenMarks,
): boolean =>
	pastMark(modifiedAt, marks[number]) || (readUnseen(projectId)[number]?.length ?? 0) > 0;

/**
 * Whether one review has unread activity — a boolean, so a watermark moving
 * on another review leaves this subscriber alone.
 */
export const useReviewUnread = (
	projectId: string,
	review: { number: number; modifiedAt: string | null },
	enabled: boolean,
): boolean => {
	const { number, modifiedAt } = review;
	return useSyncExternalStore(enabled ? subscribeMarks : subscribeNothing, () =>
		enabled ? isUnread(projectId, number, modifiedAt, readMarks(projectId)) : false,
	);
};

/** How many of `reviews` have unread activity — again a primitive. */
export const useUnreadReviewCount = (
	projectId: string,
	reviews: Array<{ number: number; modifiedAt: string | null }>,
	enabled: boolean,
): number =>
	useSyncExternalStore(enabled ? subscribeMarks : subscribeNothing, () => {
		if (!enabled) return 0;
		const marks = readMarks(projectId);
		return reviews.filter((review) => isUnread(projectId, review.number, review.modifiedAt, marks))
			.length;
	});

/**
 * Stamp reviews seen when first listed and prune marks for delisted ones.
 * An absent mark reads as seen, so the stamp is what makes only activity
 * after first sight count as unread. Mounted once per project surface.
 */
export const useStampReviewsSeen = (projectId: string): void => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	// Unconditional: behind `&&` the hook count would change mid-mount.
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
		const marks = readMarks(projectId);
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
		if (stamped || pruned) writeMarks(projectId, next);

		const unseen = readUnseen(projectId);
		const keptUnseen: UnseenEntries = {};
		for (const [number, entries] of Object.entries(unseen))
			if (Number(number) in next) keptUnseen[Number(number)] = entries;
		if (Object.keys(keptUnseen).length !== Object.keys(unseen).length)
			writeUnseen(projectId, keptUnseen);
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
	const { data: selfLogin } = useQuery(currentForgeLoginQueryOptions(projectId));
	const level = usePrNotificationsLevel();
	const [sinceMs] = useState(() => {
		const mark = readMarks(projectId)[reviewNumber];
		const ms = mark === undefined ? Number.NaN : Date.parse(mark);
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
export const markReviewSeen = (projectId: string, number: number, modifiedAt: string): void => {
	const floor = readMarks(projectId)[number];
	const floorMs = floor === undefined ? Infinity : Date.parse(floor);
	const slot = reviewSlot(projectId, number);
	const pending = pendingSeen.get(slot) ?? new Set<string>();
	const skipped = new Map((readUnseen(projectId)[number] ?? []).map(([k, at]) => [k, at]));
	for (const item of [...(shownItems.get(slot)?.values() ?? [])].flat()) {
		if (item.atMs > floorMs && !pending.has(item.key) && !skipped.has(item.key))
			skipped.set(item.key, new Date(item.atMs).toISOString());
	}
	pendingSeen.delete(slot);
	const entries = [...skipped]
		.sort(([, a], [, b]) => Date.parse(a) - Date.parse(b))
		// Beyond the cap the oldest are dropped: reading as seen is the safe
		// failure, and a skip that old was never getting read.
		.slice(-unseenCap);
	const nextUnseen = { ...readUnseen(projectId) };
	if (entries.length > 0) nextUnseen[number] = entries;
	else delete nextUnseen[number];
	// One notify for both writes: each fans out to a snapshot per row.
	setUnseen(projectId, nextUnseen);
	writeMarks(projectId, { ...readMarks(projectId), [number]: modifiedAt });
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
	const { number, modifiedAt } = review;
	// Nothing unread means nothing to write, on remount or with tracking off.
	const behind = useReviewUnread(projectId, review, enabled);

	const mark = useEffectEvent(() => {
		if (modifiedAt === null || !behind || !document.hasFocus()) return;
		markReviewSeen(projectId, number, modifiedAt);
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
