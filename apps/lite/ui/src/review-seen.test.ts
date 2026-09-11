/** @vitest-environment jsdom */
import "fake-indexeddb/auto";
import { QueryClient } from "@tanstack/react-query";
import { reviewStateQueryOptions, updateReviewState } from "./review-state.ts";
const client = new QueryClient();
import {
	isItemSkipped,
	markItemSeen,
	markReviewSeen,
	reviewsSeenUpTo,
	registerReviewItems,
} from "./review-seen.ts";
import { describe, expect, it } from "vitest";

let nextProject = 0;
const freshProject = () => `test-project-${nextProject++}`;

const unseenOf = (projectId: string) =>
	client.getQueryData(reviewStateQueryOptions(projectId).queryKey)?.unseen ?? {};
const marksOf = (projectId: string) =>
	client.getQueryData(reviewStateQueryOptions(projectId).queryKey)?.marks ?? {};

const markReviewsSeenUpTo = (projectId: string, latest: Iterable<readonly [number, string]>) =>
	updateReviewState(client, projectId, (state) => reviewsSeenUpTo(state, latest));

const at = (minute: number) => `2026-08-30T10:${String(minute).padStart(2, "0")}:00.000Z`;
const atMs = (minute: number) => Date.parse(at(minute));

describe("markReviewSeen", () => {
	it("records registered items above the old watermark as skips", async () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(0) }));
		registerReviewItems(projectId, 7, "conversation", [
			{ key: "c:1", atMs: atMs(5) },
			{ key: "c:2", atMs: atMs(10) },
		]);

		await markReviewSeen(client, projectId, 7, at(15));

		expect(marksOf(projectId)[7]).toBe(at(15));
		expect(unseenOf(projectId)[7]?.map(([key]) => key)).toEqual(["c:1", "c:2"]);
	});

	it("does not skip items already below the watermark, or looked at first", async () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(6) }));
		registerReviewItems(projectId, 7, "conversation", [
			{ key: "c:1", atMs: atMs(5) },
			{ key: "c:2", atMs: atMs(10) },
			{ key: "c:3", atMs: atMs(12) },
		]);
		// Looked at before the dwell fired: pre-empted, never a skip.
		await markItemSeen(client, projectId, 7, "c:3");

		await markReviewSeen(client, projectId, 7, at(15));

		expect(unseenOf(projectId)[7]?.map(([key]) => key)).toEqual(["c:2"]);
	});

	it("keeps existing skips across further advances, capped oldest-first", async () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(0) }));
		registerReviewItems(
			projectId,
			7,
			"conversation",
			Array.from({ length: 55 }, (_, i) => ({ key: `c:${i}`, atMs: atMs(1) + i * 1000 })),
		);

		await markReviewSeen(client, projectId, 7, at(15));

		const kept = unseenOf(projectId)[7] ?? [];
		expect(kept).toHaveLength(50);
		// The oldest five dropped: reading as seen is the safe failure.
		expect(kept[0]?.[0]).toBe("c:5");
	});

	it("merges what several surfaces registered", async () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(0) }));
		registerReviewItems(projectId, 7, "conversation", [{ key: "c:1", atMs: atMs(5) }]);
		registerReviewItems(projectId, 7, "timeline", [{ key: "e:committed:x", atMs: atMs(6) }]);

		await markReviewSeen(client, projectId, 7, at(15));

		expect(unseenOf(projectId)[7]?.map(([key]) => key)).toEqual(["c:1", "e:committed:x"]);
	});
});

describe("markItemSeen", () => {
	it("clears a recorded skip, and the review with the last of them", async () => {
		const projectId = freshProject();
		localStorage.setItem(
			`pr_activity_unseen:v1:${projectId}`,
			JSON.stringify({
				7: [
					["c:1", at(5)],
					["c:2", at(10)],
				],
			}),
		);

		expect(
			isItemSkipped(await client.fetchQuery(reviewStateQueryOptions(projectId)), 7, "c:1"),
		).toBe(true);
		await markItemSeen(client, projectId, 7, "c:1");
		expect(
			isItemSkipped(await client.fetchQuery(reviewStateQueryOptions(projectId)), 7, "c:1"),
		).toBe(false);
		expect(unseenOf(projectId)[7]?.map(([key]) => key)).toEqual(["c:2"]);

		await markItemSeen(client, projectId, 7, "c:2");
		expect(unseenOf(projectId)[7]).toBeUndefined();
	});
});

describe("markReviewsSeenUpTo", () => {
	it("advances each watermark monotonically and drops the review's skips", async () => {
		const projectId = freshProject();
		localStorage.setItem(
			`pr_activity_seen:v1:${projectId}`,
			JSON.stringify({ 7: at(0), 8: at(30), 9: at(0) }),
		);
		localStorage.setItem(
			`pr_activity_unseen:v1:${projectId}`,
			JSON.stringify({ 7: [["c:1", at(5)]], 9: [["c:2", at(5)]] }),
		);

		await markReviewsSeenUpTo(
			projectId,
			new Map([
				[7, at(10)],
				[8, at(10)],
			]),
		);

		expect(marksOf(projectId)).toEqual({ 7: at(10), 8: at(30), 9: at(0) });
		expect(
			isItemSkipped(await client.fetchQuery(reviewStateQueryOptions(projectId)), 7, "c:1"),
		).toBe(false);
		expect(
			isItemSkipped(await client.fetchQuery(reviewStateQueryOptions(projectId)), 9, "c:2"),
		).toBe(true);
	});

	it("clears skips at or before the cutoff and keeps those after it", async () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(0) }));
		localStorage.setItem(
			`pr_activity_unseen:v1:${projectId}`,
			JSON.stringify({
				7: [
					["c:1", at(5)],
					["c:2", at(10)],
					["c:3", at(15)],
				],
			}),
		);

		// Repeated stamps settle on the newest as the cutoff, in either order.
		await markReviewsSeenUpTo(projectId, [
			[7, at(10)],
			[7, at(5)],
		]);

		expect(marksOf(projectId)[7]).toBe(at(10));
		expect(unseenOf(projectId)[7]?.map(([key]) => key)).toEqual(["c:3"]);
	});

	it("settles repeated stamps on the newest, and writes nothing when nothing changes", async () => {
		const projectId = freshProject();
		localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: at(0) }));

		await markReviewsSeenUpTo(projectId, [
			[7, at(10)],
			[7, at(5)],
		]);
		expect(marksOf(projectId)[7]).toBe(at(10));

		const previous = await client.fetchQuery(reviewStateQueryOptions(projectId));
		expect(reviewsSeenUpTo(previous, [[7, at(5)]])).toBe(previous);
		const withSkip = { ...previous, unseen: { 7: [["c:1", at(5)] as [string, string]] } };
		const next = reviewsSeenUpTo(withSkip, [[7, at(10)]]);
		expect(next.unseen).toEqual({});
		expect(next.marks).toEqual(previous.marks);
	});
});
