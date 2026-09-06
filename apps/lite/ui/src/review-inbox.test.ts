/** @vitest-environment jsdom */
import { addInboxEntries, markInboxSeen, type InboxEntry } from "./review-inbox.ts";
import { describe, expect, it } from "vitest";

/**
 * The module caches per storage key, so each test gets its own project id
 * rather than sharing a window-wide reset.
 */
let nextProject = 0;
const freshProject = () => `test-project-${nextProject++}`;

const stored = (projectId: string): Array<InboxEntry> =>
	JSON.parse(
		localStorage.getItem(`pr_activity_inbox:v1:${projectId}`) ?? "[]",
	) as Array<InboxEntry>;

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
	it("keeps the list ordered by each entry's own time, across polls", () => {
		const projectId = freshProject();
		addInboxEntries(projectId, [entry("a", 30)]);
		// A later poll files an entry whose newest item is older — it must
		// sort by its time, not jump the queue for arriving late.
		addInboxEntries(projectId, [entry("b", 10), entry("c", 40)]);

		expect(stored(projectId).map((e) => e.id)).toEqual(["c", "a", "b"]);
	});

	it("leaves an already-filed id exactly where it is, seen state and all", () => {
		const projectId = freshProject();
		addInboxEntries(projectId, [entry("a", 10)]);
		markInboxSeen(projectId, ["a"]);
		// The review bumped again but this kind's bucket did not change.
		addInboxEntries(projectId, [entry("a", 10), entry("b", 20)]);

		expect(stored(projectId).map((e) => [e.id, e.seen])).toEqual([
			["b", false],
			["a", true],
		]);
	});

	it("drops the oldest past the cap", () => {
		const projectId = freshProject();
		addInboxEntries(
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

const marksOf = (projectId: string): Record<string, string> =>
	JSON.parse(localStorage.getItem(`pr_activity_seen:v1:${projectId}`) ?? "{}") as Record<
		string,
		string
	>;
const unseenOf = (projectId: string): Record<string, unknown> =>
	JSON.parse(localStorage.getItem(`pr_activity_unseen:v1:${projectId}`) ?? "{}") as Record<
		string,
		unknown
	>;

describe("markInboxSeen without ids", () => {
	it("advances each review's watermark to its newest inbox entry, clearing its skips", () => {
		const projectId = freshProject();
		localStorage.setItem(
			`pr_activity_seen:v1:${projectId}`,
			JSON.stringify({ 7: at(0), 8: at(0) }),
		);
		localStorage.setItem(
			`pr_activity_unseen:v1:${projectId}`,
			JSON.stringify({ 7: [["c:1", at(3)]], 9: [["c:2", at(3)]] }),
		);
		addInboxEntries(projectId, [entry("a", 5), entry("b", 10), entry("c", 20, { review: 8 })]);

		markInboxSeen(projectId);

		expect(stored(projectId).every((e) => e.seen)).toBe(true);
		expect(marksOf(projectId)).toEqual({ 7: at(10), 8: at(20) });
		// Activity after the newest entry is not what the reader declared read.
		expect(Date.parse(marksOf(projectId)[7] ?? "")).toBeLessThan(Date.parse(at(12)));
		// Review 9 has no entry in the inbox: its skip is not this action's to clear.
		expect(unseenOf(projectId)).toEqual({ 9: [["c:2", at(3)]] });
	});

	it("still advances a stale watermark when every entry is already seen", () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(0) }));
		addInboxEntries(projectId, [entry("a", 10, { seen: true })]);

		markInboxSeen(projectId);

		expect(marksOf(projectId)[7]).toBe(at(10));
	});

	it("never moves a watermark backwards", () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(30) }));
		addInboxEntries(projectId, [entry("a", 10)]);

		markInboxSeen(projectId);

		expect(marksOf(projectId)[7]).toBe(at(30));
	});

	it("leaves other projects alone", () => {
		const projectId = freshProject();
		const other = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${other}`, JSON.stringify({ 7: at(0) }));
		addInboxEntries(other, [entry("a", 10)]);
		addInboxEntries(projectId, [entry("a", 10)]);

		markInboxSeen(projectId);

		expect(marksOf(other)[7]).toBe(at(0));
		expect(stored(other)[0]?.seen).toBe(false);
	});
});

describe("markInboxSeen with ids", () => {
	it("marks only those entries, leaving review watermarks and skips as they were", () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(0) }));
		localStorage.setItem(
			`pr_activity_unseen:v1:${projectId}`,
			JSON.stringify({ 7: [["c:1", at(3)]] }),
		);
		addInboxEntries(projectId, [entry("a", 5), entry("b", 10)]);

		markInboxSeen(projectId, ["b"]);

		expect(stored(projectId).map((e) => [e.id, e.seen])).toEqual([
			["b", true],
			["a", false],
		]);
		expect(marksOf(projectId)).toEqual({ 7: at(0) });
		expect(unseenOf(projectId)).toEqual({ 7: [["c:1", at(3)]] });
	});
});
