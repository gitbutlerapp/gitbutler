/**
 * @file The notification inbox behind the bell.
 *
 * One entry per review, kind and poll — the detector writes what happened,
 * structured, and the bell renders it richly. Same store discipline as the
 * watermarks: local storage, disposable, capped, validated at parse, pure
 * in-memory snapshots.
 */

import { useSyncExternalStore } from "react";

export type InboxKind =
	| "comment"
	| "mention"
	| "approved"
	| "changesRequested"
	| "reviewRequested"
	| "committed"
	| "merged"
	| "closed";

export type InboxEntry = {
	id: string;
	kind: InboxKind;
	review: number;
	reviewTitle: string;
	unitSymbol: string;
	sourceBranch: string;
	htmlUrl: string;
	author: string | null;
	/** How many items this entry coalesces — "3 comments". */
	count: number;
	snippet: string | null;
	at: string;
	seen: boolean;
};

const inboxKinds: ReadonlyArray<string> = [
	"comment",
	"mention",
	"approved",
	"changesRequested",
	"reviewRequested",
	"committed",
	"merged",
	"closed",
];

const storageKey = (projectId: string) => `pr_activity_inbox:v1:${projectId}`;

/** Newest kept; the overflow was old news nobody opened. */
const inboxCap = 100;

const listeners = new Set<() => void>();
let cached: { key: string; entries: Array<InboxEntry> } | null = null;

// Storage can throw — disabled, partitioned, or over quota — and a throw
// here would crash every subscriber's render. The inbox degrades instead.
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

const parseEntries = (raw: string | null): Array<InboxEntry> => {
	if (raw === null) return [];
	try {
		const stored: unknown = JSON.parse(raw);
		if (!Array.isArray(stored)) return [];
		// Ordered here, not just at write time: a list stored by an older
		// build keeps whatever order it had until something new files.
		return stored
			.filter((entry): entry is InboxEntry => {
				if (typeof entry !== "object" || entry === null) return false;
				const e = entry as Record<string, unknown>;
				return (
					typeof e.id === "string" &&
					typeof e.kind === "string" &&
					inboxKinds.includes(e.kind) &&
					typeof e.review === "number" &&
					typeof e.reviewTitle === "string" &&
					typeof e.unitSymbol === "string" &&
					typeof e.sourceBranch === "string" &&
					typeof e.htmlUrl === "string" &&
					(e.author === null || typeof e.author === "string") &&
					typeof e.count === "number" &&
					(e.snippet === null || typeof e.snippet === "string") &&
					typeof e.at === "string" &&
					!Number.isNaN(Date.parse(e.at)) &&
					typeof e.seen === "boolean"
				);
			})
			.sort((a, b) => Date.parse(b.at) - Date.parse(a.at))
			.slice(0, inboxCap);
	} catch {
		return [];
	}
};

const readEntries = (projectId: string): Array<InboxEntry> => {
	const key = storageKey(projectId);
	if (cached?.key === key) return cached.entries;
	const entries = parseEntries(storageGet(key));
	cached = { key, entries };
	return entries;
};

const notify = (): void => {
	for (const listener of listeners) listener();
};

const writeEntries = (projectId: string, entries: Array<InboxEntry>): void => {
	storageSet(storageKey(projectId), JSON.stringify(entries));
	cached = { key: storageKey(projectId), entries };
	notify();
};

let watchingStorage = false;

const onStorage = (event: StorageEvent): void => {
	// A null key is a wholesale clear; other keys are someone else's business.
	if (event.key !== null && !event.key.startsWith("pr_activity_")) return;
	cached = null;
	notify();
};

const subscribeInbox = (listener: () => void): (() => void) => {
	// On first use, so merely importing this module listens to nothing.
	if (!watchingStorage) {
		watchingStorage = true;
		window.addEventListener("storage", onStorage);
	}
	// A first subscriber is a fresh surface: re-read whatever storage holds.
	if (listeners.size === 0) cached = null;
	listeners.add(listener);
	return () => listeners.delete(listener);
};

const subscribeNothing = (): (() => void) => () => {};

/**
 * File the poll's entries, keeping the list ordered by each entry's own
 * time. An id already filed is left exactly where it is, seen state and
 * all — a review bumping again must not resurface an old entry as new.
 */
export const addInboxEntries = (projectId: string, entries: Array<InboxEntry>): void => {
	if (entries.length === 0) return;
	const existing = readEntries(projectId);
	const known = new Set(existing.map((entry) => entry.id));
	const fresh = entries.filter((entry) => !known.has(entry.id));
	if (fresh.length === 0) return;
	const next = [...fresh, ...existing]
		.sort((a, b) => Date.parse(b.at) - Date.parse(a.at))
		.slice(0, inboxCap);
	writeEntries(projectId, next);
};

/** Mark the given entries seen; without ids, everything. */
export const markInboxSeen = (projectId: string, ids?: ReadonlyArray<string>): void => {
	const entries = readEntries(projectId);
	const wanted = ids === undefined ? null : new Set(ids);
	if (!entries.some((entry) => !entry.seen && (wanted === null || wanted.has(entry.id)))) return;
	writeEntries(
		projectId,
		entries.map((entry) =>
			!entry.seen && (wanted === null || wanted.has(entry.id)) ? { ...entry, seen: true } : entry,
		),
	);
};

/** The inbox, newest first — a stable array identity between writes. */
export const useInboxEntries = (projectId: string, enabled: boolean): Array<InboxEntry> =>
	useSyncExternalStore(enabled ? subscribeInbox : subscribeNothing, () =>
		enabled ? readEntries(projectId) : emptyInbox,
	);

const emptyInbox: Array<InboxEntry> = [];

/** How many entries are unseen — the bell's dot. */
export const useInboxUnseenCount = (projectId: string, enabled: boolean): number =>
	useSyncExternalStore(enabled ? subscribeInbox : subscribeNothing, () =>
		enabled ? readEntries(projectId).filter((entry) => !entry.seen).length : 0,
	);
