import { queryOptions, type QueryClient } from "@tanstack/react-query";
import * as idb from "idb-keyval";
import { isAgent } from "#ui/review-users.ts";

export type InboxEntry = {
	id: string;
	legacyId?: string;
	kind: InboxKind;
	review: number;
	reviewTitle: string;
	unitSymbol: string;
	sourceBranch: string;
	htmlUrl: string;
	author: string | null;
	authorIsBot?: boolean;
	/** How many items this entry coalesces — "3 comments". */
	count: number;
	/** The comment a click should land on, when the entry is about comments. */
	commentId?: number | null;
	snippet: string | null;
	at: string;
	seen: boolean;
};

const inboxKinds = [
	"comment",
	"mention",
	"approved",
	"changesRequested",
	"reviewRequested",
	"committed",
	"merged",
	"closed",
] as const;
export type InboxKind = (typeof inboxKinds)[number];

export type SeenMarks = Record<number, string>;
// The complement stays bounded even when one skipped item pins an old watermark.
export type UnseenEntries = Record<number, Array<[key: string, at: string]>>;
export const unseenCap = 50;

export type ReviewState = {
	marks: SeenMarks;
	unseen: UnseenEntries;
	inbox: Array<InboxEntry>;
};

export const isBotEntry = (entry: InboxEntry): boolean =>
	isAgent({ login: entry.author ?? "", isBot: entry.authorIsBot === true });

/** Each tab keeps its own history so agent traffic cannot crowd out humans. */
const inboxCapPerType = 100;

export const capInboxEntries = (entries: Array<InboxEntry>): Array<InboxEntry> => {
	let humans = 0;
	let agents = 0;
	return entries.filter((entry) =>
		isBotEntry(entry) ? ++agents <= inboxCapPerType : ++humans <= inboxCapPerType,
	);
};

const parseMarks = (raw: string | null): SeenMarks => {
	if (raw === null) return {};
	try {
		const stored: unknown = JSON.parse(raw);
		if (typeof stored !== "object" || stored === null || Array.isArray(stored)) return {};
		const marks: SeenMarks = {};
		for (const [reviewNumberText, seenAt] of Object.entries(stored)) {
			if (
				typeof seenAt === "string" &&
				/^\d+$/.test(reviewNumberText) &&
				!Number.isNaN(Date.parse(seenAt))
			)
				marks[Number(reviewNumberText)] = seenAt;
		}
		return marks;
	} catch {
		return {};
	}
};

const parseUnseen = (raw: string | null): UnseenEntries => {
	if (raw === null) return {};
	try {
		const stored: unknown = JSON.parse(raw);
		if (typeof stored !== "object" || stored === null || Array.isArray(stored)) return {};
		const entries: UnseenEntries = {};
		for (const [reviewNumberText, unseenItems] of Object.entries(stored)) {
			if (!/^\d+$/.test(reviewNumberText) || !Array.isArray(unseenItems)) continue;
			const kept = unseenItems.filter(
				(entry): entry is [string, string] =>
					Array.isArray(entry) &&
					typeof entry[0] === "string" &&
					typeof entry[1] === "string" &&
					!Number.isNaN(Date.parse(entry[1])),
			);
			if (kept.length > 0) entries[Number(reviewNumberText)] = kept.slice(-unseenCap);
		}
		return entries;
	} catch {
		return {};
	}
};

const migrateEntries = (entries: Array<InboxEntry>): Array<InboxEntry> => {
	const byId = new Map<string, InboxEntry>();
	for (const stored of entries) {
		const legacyAgent =
			stored.authorIsBot === undefined &&
			isBotEntry(stored) &&
			stored.id === `${stored.review}:${stored.kind}:${stored.at}`;
		const entry = legacyAgent ? { ...stored, id: `${stored.id}:bot`, legacyId: stored.id } : stored;
		const previous = byId.get(entry.id);
		byId.set(
			entry.id,
			previous
				? {
						...entry,
						seen: previous.seen || entry.seen,
						authorIsBot: entry.authorIsBot ?? previous.authorIsBot,
						legacyId: entry.legacyId ?? previous.legacyId,
					}
				: entry,
		);
	}
	return [...byId.values()].map((entry) =>
		entry.legacyId !== undefined && byId.has(entry.legacyId)
			? { ...entry, legacyId: undefined }
			: entry,
	);
};

const parseEntries = (raw: string | null): Array<InboxEntry> => {
	if (raw === null) return [];
	try {
		const stored: unknown = JSON.parse(raw);
		if (!Array.isArray(stored)) return [];
		// Ordered here, not just at write time: a list stored by an older
		// build keeps whatever order it had until something new files.
		return capInboxEntries(
			migrateEntries(
				stored.filter((entry): entry is InboxEntry => {
					if (typeof entry !== "object" || entry === null) return false;
					const e = entry as Record<string, unknown>;
					return (
						typeof e.id === "string" &&
						(e.legacyId === undefined || typeof e.legacyId === "string") &&
						typeof e.kind === "string" &&
						inboxKinds.some((kind) => kind === e.kind) &&
						typeof e.review === "number" &&
						typeof e.reviewTitle === "string" &&
						typeof e.unitSymbol === "string" &&
						typeof e.sourceBranch === "string" &&
						typeof e.htmlUrl === "string" &&
						(e.author === null || typeof e.author === "string") &&
						(e.authorIsBot === undefined || typeof e.authorIsBot === "boolean") &&
						typeof e.count === "number" &&
						(e.commentId === undefined ||
							e.commentId === null ||
							typeof e.commentId === "number") &&
						(e.snippet === null || typeof e.snippet === "string") &&
						typeof e.at === "string" &&
						!Number.isNaN(Date.parse(e.at)) &&
						typeof e.seen === "boolean"
					);
				}),
			).sort((a, b) => Date.parse(b.at) - Date.parse(a.at)),
		);
	} catch {
		return [];
	}
};

const reviewStore = idb.createStore("keyval-store", "keyval");
const storageKey = (projectId: string) => `pr_activity:v1:${projectId}`;
const legacyKey = (projectId: string, part: string) => `pr_activity_${part}:v1:${projectId}`;
const readLegacy = (projectId: string): ReviewState => {
	const read = (part: string) => {
		try {
			return localStorage.getItem(legacyKey(projectId, part));
		} catch {
			return null;
		}
	};
	return {
		marks: parseMarks(read("seen")),
		unseen: parseUnseen(read("unseen")),
		inbox: parseEntries(read("inbox")),
	};
};

export const reviewStateQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: [projectId, "reviewState"],
		staleTime: Infinity,
		queryFn: async ({ client }): Promise<ReviewState> => {
			try {
				const stored = await idb.get<ReviewState>(storageKey(projectId));
				if (stored !== undefined) return stored;
				let migrated!: ReviewState;
				// Two windows may migrate together; only the first initializes the record.
				await idb.update<ReviewState>(
					storageKey(projectId),
					(current) => (migrated = current ?? readLegacy(projectId)),
				);
				try {
					for (const part of ["seen", "unseen", "inbox"])
						localStorage.removeItem(legacyKey(projectId, part));
				} catch {
					/* The durable record takes precedence if legacy storage is unavailable. */
				}
				return migrated;
			} catch {
				// Tracking remains usable for this session when persistence is unavailable.
				return (
					client.getQueryData<ReviewState>([projectId, "reviewState"]) ?? readLegacy(projectId)
				);
			}
		},
	});

export const updateReviewState = async (
	client: QueryClient,
	projectId: string,
	update: (state: ReviewState) => ReviewState,
): Promise<ReviewState> => {
	const options = reviewStateQueryOptions(projectId);
	const loaded = await client.ensureQueryData(options);
	let next!: ReviewState;
	let changed = false;
	try {
		// Compare and write in one transaction so stale windows cannot lose real updates.
		await reviewStore("readwrite", async (store) => {
			const current =
				(await idb.promisifyRequest<ReviewState | undefined>(store.get(storageKey(projectId)))) ??
				loaded;
			next = update(current);
			changed = next !== current;
			if (changed) store.put(next, storageKey(projectId));
			return idb.promisifyRequest(store.transaction);
		});
	} catch {
		const current = client.getQueryData(options.queryKey) ?? loaded;
		next = update(current);
		changed = next !== current;
	}
	if (!changed) return next;
	await client.cancelQueries({ queryKey: options.queryKey });
	client.setQueryData(options.queryKey, next);
	return next;
};

export const watchReviewState = (client: QueryClient): (() => void) => {
	const channel = new BroadcastChannel("pr_activity:v1");
	channel.onmessage = ({ data }: MessageEvent<unknown>) => {
		if (typeof data === "string")
			void client.invalidateQueries({ queryKey: reviewStateQueryOptions(data).queryKey });
	};
	const unsubscribe = client.getQueryCache().subscribe((event) => {
		if (event.type !== "updated" || event.action.type !== "success" || !event.action.manual) return;
		const [projectId, name] = event.query.queryKey as ReadonlyArray<unknown>;
		if (name === "reviewState" && typeof projectId === "string") channel.postMessage(projectId);
	});
	return () => {
		unsubscribe();
		channel.close();
	};
};
