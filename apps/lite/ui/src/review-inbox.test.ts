/** @vitest-environment jsdom */
import "fake-indexeddb/auto";
import { QueryClient } from "@tanstack/react-query";
import { reviewStateQueryOptions } from "./review-state.ts";
const client = new QueryClient();
import {
	addInboxEntries,
	desktopNotices,
	entryHeadline,
	findInboxEntry,
	isBotEntry,
	markInboxSeen,
	summaryNoticeId,
	type InboxEntry,
} from "./review-inbox.ts";
import { describe, expect, it } from "vitest";

let nextProject = 0;
const freshProject = () => `test-project-${nextProject++}`;

const stored = (projectId: string): Array<InboxEntry> =>
	client.getQueryData(reviewStateQueryOptions(projectId).queryKey)?.inbox ?? [];

const at = (minute: number) => `2026-08-30T10:${String(minute).padStart(2, "0")}:00.000Z`;

const entry = (id: string, minute: number, overrides: Partial<InboxEntry> = {}): InboxEntry => ({
	id,
	kind: "comment",
	review: 7,
	reviewTitle: "A fixture change",
	unitSymbol: "#",
	sourceBranch: "feature-one",
	htmlUrl: "https://example.com/7",
	author: "alice",
	count: 1,
	snippet: null,
	at: at(minute),
	seen: false,
	...overrides,
});

describe("addInboxEntries", () => {
	it("keeps the list ordered by each entry's own time, across polls", async () => {
		const projectId = freshProject();
		await addInboxEntries(client, projectId, [entry("a", 30)]);
		// A later poll files an entry whose newest item is older — it must
		// sort by its time, not jump the queue for arriving late.
		await addInboxEntries(client, projectId, [entry("b", 10), entry("c", 40)]);

		expect(stored(projectId).map((e) => e.id)).toEqual(["c", "a", "b"]);
	});

	it("leaves an already-filed id exactly where it is, seen state and all", async () => {
		const projectId = freshProject();
		await addInboxEntries(client, projectId, [entry("a", 10)]);
		await markInboxSeen(client, projectId, ["a"]);
		// The review bumped again but this kind's bucket did not change.
		await addInboxEntries(client, projectId, [entry("a", 10), entry("b", 20)]);

		expect(stored(projectId).map((e) => [e.id, e.seen])).toEqual([
			["b", false],
			["a", true],
		]);
	});

	it("returns only the entries that were new", async () => {
		const projectId = freshProject();
		expect((await addInboxEntries(client, projectId, [entry("a", 10)])).map((e) => e.id)).toEqual([
			"a",
		]);
		expect(
			(await addInboxEntries(client, projectId, [entry("a", 10), entry("b", 20)])).map((e) => e.id),
		).toEqual(["b"]);
	});

	it("drops the oldest past the cap", async () => {
		const projectId = freshProject();
		await addInboxEntries(
			client,
			projectId,
			Array.from({ length: 105 }, (_, i) =>
				entry(`e${i}`, 0, { at: new Date(1756500000000 + i * 60000).toISOString() }),
			),
		);

		const kept = stored(projectId);
		expect(kept).toHaveLength(100);
		expect(kept[0]?.id).toBe("e104");
		expect(kept[99]?.id).toBe("e5");
	});
});

describe("retention per notification type", () => {
	const batch = (authorIsBot: boolean, offset: number) =>
		Array.from({ length: 105 }, (_, i) =>
			entry(`${authorIsBot ? "agent" : "human"}-${i}`, 0, {
				authorIsBot,
				at: new Date(1756500000000 + (offset + i) * 60000).toISOString(),
			}),
		);

	it.each([false, true])(
		"keeps older notifications when the other type floods the inbox (agents first: %s)",
		async (agentsFirst) => {
			const projectId = freshProject();
			await addInboxEntries(client, projectId, batch(agentsFirst, 0));
			await addInboxEntries(client, projectId, batch(!agentsFirst, 200));
			const kept = stored(projectId);
			expect(kept).toHaveLength(200);
			expect(kept.filter(isBotEntry)).toHaveLength(100);
			expect(kept.filter((item) => !isBotEntry(item))).toHaveLength(100);
			expect(kept.some((item) => item.id.endsWith("-0"))).toBe(false);
			expect(kept.every((item, i) => i === 0 || item.at <= (kept[i - 1]?.at ?? item.at))).toBe(
				true,
			);
		},
	);

	it("retains both types when reading persisted entries", async () => {
		const projectId = freshProject();
		localStorage.setItem(
			`pr_activity_inbox:v1:${projectId}`,
			JSON.stringify([...batch(false, 0), ...batch(true, 200)]),
		);
		await markInboxSeen(client, projectId);
		const kept = stored(projectId);
		expect(kept).toHaveLength(200);
		expect(kept.filter(isBotEntry)).toHaveLength(100);
		expect(kept.every((item) => item.seen)).toBe(true);
		expect(kept[0]?.id).toBe("agent-104");
		expect(kept[199]?.id).toBe("human-5");
	});
});

const marksOf = (projectId: string) =>
	client.getQueryData(reviewStateQueryOptions(projectId).queryKey)?.marks ?? {};
const unseenOf = (projectId: string) =>
	client.getQueryData(reviewStateQueryOptions(projectId).queryKey)?.unseen ?? {};

describe("markInboxSeen without ids", () => {
	it("advances each review's watermark to its newest inbox entry, clearing its skips", async () => {
		const projectId = freshProject();
		localStorage.setItem(
			`pr_activity_seen:v1:${projectId}`,
			JSON.stringify({ 7: at(0), 8: at(0) }),
		);
		localStorage.setItem(
			`pr_activity_unseen:v1:${projectId}`,
			JSON.stringify({ 7: [["c:1", at(3)]], 9: [["c:2", at(3)]] }),
		);
		await addInboxEntries(client, projectId, [
			entry("a", 5),
			entry("b", 10),
			entry("c", 20, { review: 8 }),
		]);

		await markInboxSeen(client, projectId);

		expect(stored(projectId).every((e) => e.seen)).toBe(true);
		expect(marksOf(projectId)).toEqual({ 7: at(10), 8: at(20) });
		// Activity after the newest entry is not what the reader declared read.
		expect(Date.parse(marksOf(projectId)[7] ?? "")).toBeLessThan(Date.parse(at(12)));
		// Review 9 has no entry in the inbox: its skip is not this action's to clear.
		expect(unseenOf(projectId)).toEqual({ 9: [["c:2", at(3)]] });
	});

	it("still advances a stale watermark when every entry is already seen", async () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(0) }));
		await addInboxEntries(client, projectId, [entry("a", 10, { seen: true })]);

		await markInboxSeen(client, projectId);

		expect(marksOf(projectId)[7]).toBe(at(10));
	});

	it("never moves a watermark backwards", async () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(30) }));
		await addInboxEntries(client, projectId, [entry("a", 10)]);

		await markInboxSeen(client, projectId);

		expect(marksOf(projectId)[7]).toBe(at(30));
	});

	it("leaves other projects alone", async () => {
		const projectId = freshProject();
		const other = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${other}`, JSON.stringify({ 7: at(0) }));
		await addInboxEntries(client, other, [entry("a", 10)]);
		await addInboxEntries(client, projectId, [entry("a", 10)]);

		await markInboxSeen(client, projectId);

		expect(marksOf(other)[7]).toBe(at(0));
		expect(stored(other)[0]?.seen).toBe(false);
	});
});

describe("markInboxSeen with ids", () => {
	it("marks only those entries, leaving review watermarks and skips as they were", async () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(0) }));
		localStorage.setItem(
			`pr_activity_unseen:v1:${projectId}`,
			JSON.stringify({ 7: [["c:1", at(3)]] }),
		);
		await addInboxEntries(client, projectId, [entry("a", 5), entry("b", 10)]);

		await markInboxSeen(client, projectId, ["b"]);

		expect(stored(projectId).map((e) => [e.id, e.seen])).toEqual([
			["b", true],
			["a", false],
		]);
		expect(marksOf(projectId)).toEqual({ 7: at(0) });
		expect(unseenOf(projectId)).toEqual({ 7: [["c:1", at(3)]] });
	});
});

describe("entryHeadline", () => {
	it("leads with what happened, then by whom", async () => {
		expect(entryHeadline(entry("a", 0, { kind: "changesRequested" }))).toBe(
			"Changes requested by alice",
		);
		expect(entryHeadline(entry("a", 0, { kind: "comment", count: 3 }))).toBe(
			"3 comments from alice",
		);
		expect(entryHeadline(entry("a", 0, { kind: "mention" }))).toBe("Mentioned by alice");
		expect(entryHeadline(entry("a", 0, { kind: "committed", count: 2 }))).toBe(
			"2 commits pushed by alice",
		);
	});

	it("stands alone when the actor is unknown", async () => {
		expect(entryHeadline(entry("a", 0, { kind: "merged", author: null }))).toBe("Merged");
		expect(entryHeadline(entry("a", 0, { kind: "comment", author: null }))).toBe("Comment");
	});
});

describe("desktopNotices", () => {
	it("announces loud entries one by one, with the branch and snippet as the body", async () => {
		const notices = desktopNotices([
			entry("a", 0, { kind: "approved" }),
			entry("b", 0, { kind: "comment", snippet: "Looks off to me" }),
			entry("c", 0, { kind: "committed" }),
		]);
		expect(notices).toEqual([
			{ id: "a", title: "Approved by alice", body: "feature-one #7" },
			{ id: "b", title: "Comment from alice", body: "feature-one #7\nLooks off to me" },
		]);
	});

	it("collapses a catch-up into one summary", async () => {
		const notices = desktopNotices(
			["a", "b", "c", "d"].map((id, i) => entry(id, i, { review: i % 2 === 0 ? 7 : 8 })),
		);
		expect(notices).toEqual([
			{ id: summaryNoticeId, title: "4 new notifications", body: "feature-one #7, feature-one #8" },
		]);
	});

	it("says nothing for quiet news", async () => {
		expect(desktopNotices([entry("a", 0, { kind: "merged" })])).toEqual([]);
	});
});

describe("bot notifications", () => {
	it("recognizes forge flags and legacy bot logins without hiding unknown authors", async () => {
		expect(isBotEntry(entry("flagged", 1, { author: "copilot", authorIsBot: true }))).toBe(true);
		expect(isBotEntry(entry("legacy", 1, { author: "renovate[bot]" }))).toBe(true);
		expect(isBotEntry(entry("reviewer", 1, { author: "copilot-pull-request-reviewer" }))).toBe(
			true,
		);
		expect(isBotEntry(entry("copilot", 1, { author: "Copilot" }))).toBe(true);
		expect(
			isBotEntry(entry("copilot-unflagged", 1, { author: "copilot", authorIsBot: false })),
		).toBe(true);
		expect(isBotEntry(entry("unknown", 1, { author: null }))).toBe(false);
		expect(isBotEntry(entry("human", 1))).toBe(false);
	});

	it("retains bot metadata in persisted entries", async () => {
		const projectId = freshProject();
		await addInboxEntries(client, projectId, [entry("bot", 1, { authorIsBot: true })]);
		expect(stored(projectId)[0]?.authorIsBot).toBe(true);
	});
});

describe("rediscovered activity", () => {
	it.each([false, true])(
		"deduplicates a legacy agent entry without changing its seen state (%s)",
		async (seen) => {
			const projectId = freshProject();
			const id = `7:comment:${at(1)}`;
			localStorage.setItem(
				`pr_activity_inbox:v1:${projectId}`,
				JSON.stringify([entry(id, 1, { author: "copilot-pull-request-reviewer", seen })]),
			);
			expect(
				await addInboxEntries(client, projectId, [
					entry(`${id}:bot`, 1, { author: "copilot-pull-request-reviewer", authorIsBot: true }),
				]),
			).toEqual([]);
			await addInboxEntries(client, projectId, [entry("unrelated", 2)]);
			client.removeQueries({ queryKey: reviewStateQueryOptions(projectId).queryKey });
			expect((await findInboxEntry(client, projectId, id))?.seen).toBe(seen);
		},
	);

	it("does not confuse a human entry with an agent entry at the same timestamp", async () => {
		const projectId = freshProject();
		const id = `7:comment:${at(1)}`;
		await addInboxEntries(client, projectId, [entry(id, 1)]);
		expect(
			await addInboxEntries(client, projectId, [entry(`${id}:bot`, 1, { authorIsBot: true })]),
		).toHaveLength(1);
		expect(stored(projectId)).toHaveLength(2);
	});

	it("does not announce replayed activity that falls outside the retained history", async () => {
		const projectId = freshProject();
		const old = entry("old", 0);
		await addInboxEntries(client, projectId, [old]);
		await markInboxSeen(client, projectId, [old.id]);
		await addInboxEntries(
			client,
			projectId,
			Array.from({ length: 100 }, (_, i) =>
				entry(`new-${i}`, 1, { at: new Date(Date.parse(at(1)) + i * 60000).toISOString() }),
			),
		);
		// Reopening the project must preserve retention and seen state.
		client.removeQueries({ queryKey: reviewStateQueryOptions(projectId).queryKey });
		expect(await addInboxEntries(client, projectId, [old])).toEqual([]);
		expect(stored(projectId).some((item) => item.id === old.id)).toBe(false);
	});
});

it("keeps a human notification that shares a legacy agent timestamp", async () => {
	const projectId = freshProject();
	const id = `7:comment:${at(1)}`;
	localStorage.setItem(
		`pr_activity_inbox:v1:${projectId}`,
		JSON.stringify([entry(id, 1, { author: "copilot-pull-request-reviewer", seen: true })]),
	);
	expect(await addInboxEntries(client, projectId, [entry(id, 1)])).toHaveLength(1);
	expect(stored(projectId)).toHaveLength(2);
	expect((await findInboxEntry(client, projectId, id))?.author).toBe("alice");
	expect((await findInboxEntry(client, projectId, `${id}:bot`))?.seen).toBe(true);
	client.removeQueries({ queryKey: reviewStateQueryOptions(projectId).queryKey });
	expect((await findInboxEntry(client, projectId, `${id}:bot`))?.seen).toBe(true);
});

it("merges already duplicated legacy agent entries without losing read state", async () => {
	const projectId = freshProject();
	const id = `7:comment:${at(1)}`;
	localStorage.setItem(
		`pr_activity_inbox:v1:${projectId}`,
		JSON.stringify([
			entry(`${id}:bot`, 1, { author: "copilot-pull-request-reviewer", authorIsBot: true }),
			entry(id, 1, { author: "copilot-pull-request-reviewer", seen: true }),
		]),
	);
	await addInboxEntries(client, projectId, [entry("another", 2)]);
	expect(stored(projectId)).toHaveLength(2);
	expect((await findInboxEntry(client, projectId, id))?.seen).toBe(true);
	expect((await findInboxEntry(client, projectId, id))?.authorIsBot).toBe(true);
});

it("does not resolve an evicted human desktop notice to an agent", async () => {
	const projectId = freshProject();
	const id = `7:comment:${at(0)}`;
	await addInboxEntries(client, projectId, [
		entry(id, 0),
		entry(`${id}:bot`, 0, { authorIsBot: true }),
	]);
	await addInboxEntries(
		client,
		projectId,
		Array.from({ length: 100 }, (_, i) =>
			entry(`human-${i}`, 1, { at: new Date(Date.parse(at(1)) + i * 60000).toISOString() }),
		),
	);
	expect(await findInboxEntry(client, projectId, id)).toBeUndefined();
	expect(await findInboxEntry(client, projectId, `${id}:bot`)).toBeDefined();
});

it("retires a migrated alias when a human entry claims its ID", async () => {
	const projectId = freshProject();
	const id = `7:comment:${at(0)}`;
	localStorage.setItem(
		`pr_activity_inbox:v1:${projectId}`,
		JSON.stringify([entry(id, 0, { author: "copilot-pull-request-reviewer" })]),
	);
	await addInboxEntries(client, projectId, [entry(id, 0)]);
	await addInboxEntries(
		client,
		projectId,
		Array.from({ length: 100 }, (_, i) =>
			entry(`human-${i}`, 1, { at: new Date(Date.parse(at(1)) + i * 60000).toISOString() }),
		),
	);
	client.removeQueries({ queryKey: reviewStateQueryOptions(projectId).queryKey });
	expect(await findInboxEntry(client, projectId, id)).toBeUndefined();
	expect(await findInboxEntry(client, projectId, `${id}:bot`)).toBeDefined();
});

it("finds another window's notification before its invalidation arrives", async () => {
	const projectId = freshProject();
	await client.fetchQuery(reviewStateQueryOptions(projectId));
	const notice = entry("from-another-window", 30);
	const other = new QueryClient();
	try {
		await addInboxEntries(other, projectId, [notice]);
		expect(await findInboxEntry(client, projectId, notice.id)).toEqual(notice);
	} finally {
		other.clear();
	}
});
