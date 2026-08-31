import {
	activityItems,
	attentionOf,
	seenLedger,
	itemMentions,
	observeReviews,
	type ReviewActivityItem,
} from "./review-activity.ts";
import type {
	ForgeReview,
	ForgeReviewComment,
	ForgeReviewSubmission,
	ForgeReviewTimelineEvent,
	ForgeReviewUser,
} from "@gitbutler/but-sdk";
import { describe, expect, it } from "vitest";

const user = (login: string): ForgeReviewUser => ({
	id: 1,
	login,
	name: null,
	email: null,
	avatarUrl: null,
	isBot: false,
});

const review = (overrides: Partial<ForgeReview> = {}): ForgeReview => ({
	htmlUrl: "https://forge.example/pr/7",
	number: 7,
	title: "A change",
	body: null,
	author: null,
	labels: [],
	draft: false,
	sourceBranch: "feature",
	targetBranch: "main",
	sha: "abc",
	integrationCommitShas: [],
	createdAt: "2026-08-28T09:00:00Z",
	modifiedAt: "2026-08-28T10:00:00Z",
	mergedAt: null,
	closedAt: null,
	repositorySshUrl: null,
	repositoryHttpsUrl: null,
	repoOwner: null,
	headRepoIsFork: false,
	reviewers: [],
	autoMergeEnabled: false,
	unitSymbol: "#",
	lastSyncAt: "2026-08-28T10:00:00Z",
	...overrides,
});

const comment = (author: string | null, atMs: number, body = ""): ReviewActivityItem => ({
	kind: "comment",
	author,
	body,
	atMs,
});

const verdict = (
	author: string | null,
	state: "approved" | "changesRequested" | "commented" | "dismissed",
	atMs: number,
	body: string | null = null,
): ReviewActivityItem => ({ kind: "verdict", author, state, body, atMs });

describe("attentionOf", () => {
	it("treats someone else's comment as loud", () => {
		expect(attentionOf(comment("alice", 1), "me")).toBe("loud");
	});

	it("treats the user's own activity as silent, whatever it is", () => {
		expect(attentionOf(comment("me", 1), "me")).toBe("silent");
		expect(attentionOf(verdict("me", "approved", 1), "me")).toBe("silent");
	});

	it("compares logins case-insensitively", () => {
		expect(attentionOf(comment("Me", 1), "mE")).toBe("silent");
	});

	it("treats review verdicts as loud except dismissals", () => {
		for (const state of ["approved", "changesRequested", "commented"] as const)
			expect(attentionOf(verdict("alice", state, 1), "me")).toBe("loud");

		expect(attentionOf(verdict("alice", "dismissed", 1), "me")).toBe("quiet");
	});

	it("treats a review request as loud only when it names the user", () => {
		const request = (requestedReviewer: string | null): ReviewActivityItem => ({
			kind: "reviewRequested",
			author: "alice",
			requestedReviewer,
			atMs: 1,
		});
		expect(attentionOf(request("me"), "me")).toBe("loud");
		expect(attentionOf(request("bob"), "me")).toBe("quiet");
		expect(attentionOf(request(null), "me")).toBe("quiet");
	});

	it("treats commits as quiet", () => {
		expect(attentionOf({ kind: "committed", author: "alice", atMs: 1 }, "me")).toBe("quiet");
	});

	it("stays sane without a known login: comments loud, requests quiet", () => {
		expect(attentionOf(comment("alice", 1), null)).toBe("loud");
		expect(
			attentionOf(
				{ kind: "reviewRequested", author: "alice", requestedReviewer: "bob", atMs: 1 },
				null,
			),
		).toBe("quiet");
	});
});

describe("seenLedger", () => {
	it("reports activity that arrived while the app was closed", () => {
		// The watermark trails the review: the user has not seen this yet, so
		// the very first observation must report it rather than absorb it.
		const listing = [review({ modifiedAt: "2026-08-28T11:00:00Z" })];
		const ledger = seenLedger(listing, { 7: "2026-08-28T10:00:00Z" });
		const { changed } = observeReviews(ledger, listing);
		expect(changed).toHaveLength(1);
		expect(changed[0]?.sinceMs).toBe(Date.parse("2026-08-28T10:00:00Z"));
	});

	it("stays silent for a review with no watermark", () => {
		const listing = [review()];
		expect(observeReviews(seenLedger(listing, {}), listing).changed).toEqual([]);
	});

	it("stays silent once the watermark has caught up", () => {
		const listing = [review({ modifiedAt: "2026-08-28T11:00:00Z" })];
		const ledger = seenLedger(listing, { 7: "2026-08-28T11:00:00Z" });
		expect(observeReviews(ledger, listing).changed).toEqual([]);
	});
});

describe("observeReviews", () => {
	it("never reports a review on first sight", () => {
		const listing = [review()];
		const { changed } = observeReviews(seenLedger([], {}), listing);
		expect(changed).toEqual([]);
	});

	it("reports a review whose modifiedAt moved, with the previous mark as the cut", () => {
		const before = review({ modifiedAt: "2026-08-28T10:00:00Z" });
		const after = review({ modifiedAt: "2026-08-28T11:00:00Z" });
		const { changed } = observeReviews(seenLedger([before], {}), [after]);
		expect(changed).toHaveLength(1);
		expect(changed[0]?.sinceMs).toBe(Date.parse("2026-08-28T10:00:00Z"));
		expect(changed[0]?.settledNow).toBeNull();
	});

	it("stays quiet while nothing moves", () => {
		const same = review();
		expect(observeReviews(seenLedger([same], {}), [same]).changed).toEqual([]);
	});

	it("reports the poll on which a review settles", () => {
		const open = review();
		const merged = review({
			modifiedAt: "2026-08-28T11:00:00Z",
			mergedAt: "2026-08-28T11:00:00Z",
		});
		expect(observeReviews(seenLedger([open], {}), [merged]).changed[0]?.settledNow).toBe("merged");

		const closed = review({
			modifiedAt: "2026-08-28T11:00:00Z",
			closedAt: "2026-08-28T11:00:00Z",
		});
		expect(observeReviews(seenLedger([open], {}), [closed]).changed[0]?.settledNow).toBe("closed");
	});

	it("does not re-report settling on later activity of a settled review", () => {
		const merged = review({ mergedAt: "2026-08-28T11:00:00Z" });
		const laterStill = review({
			mergedAt: "2026-08-28T11:00:00Z",
			modifiedAt: "2026-08-28T12:00:00Z",
		});
		expect(
			observeReviews(seenLedger([merged], {}), [laterStill]).changed[0]?.settledNow,
		).toBeNull();
	});

	it("forgets reviews the listing no longer carries", () => {
		const gone = review({ number: 9 });
		const { next } = observeReviews(seenLedger([gone], {}), []);
		expect(next.size).toBe(0);
	});
});

describe("activityItems", () => {
	const at = (iso: string) => Date.parse(iso);

	it("keeps only items strictly after the cut, and drops undated ones", () => {
		const comments: Array<ForgeReviewComment> = [
			{
				id: 1,
				body: "old",
				author: user("alice"),
				createdAt: "2026-08-28T09:00:00Z",
				modifiedAt: null,
				htmlUrl: "",
				reactions: [],
			},
			{
				id: 2,
				body: "new",
				author: user("alice"),
				createdAt: "2026-08-28T11:00:00Z",
				modifiedAt: null,
				htmlUrl: "",
				reactions: [],
			},
			{
				id: 3,
				body: "undated",
				author: user("alice"),
				createdAt: null,
				modifiedAt: null,
				htmlUrl: "",
				reactions: [],
			},
		];
		const items = activityItems(comments, [], [], at("2026-08-28T10:00:00Z"));
		expect(items).toHaveLength(1);
		expect(items[0]).toMatchObject({ kind: "comment", author: "alice" });
	});

	it("maps submissions and timeline events onto their kinds", () => {
		const submissions: Array<ForgeReviewSubmission> = [
			{
				id: 1,
				author: user("alice"),
				state: "changesRequested",
				body: null,
				submittedAt: "2026-08-28T11:00:00Z",
				htmlUrl: "",
			},
		];
		const events: Array<ForgeReviewTimelineEvent> = [
			{
				kind: "reviewRequested",
				actor: user("bob"),
				requestedReviewer: user("me"),
				commitSha: null,
				commitSummary: null,
				commitAuthorName: null,
				createdAt: "2026-08-28T11:30:00Z",
			},
			{
				kind: "committed",
				actor: null,
				requestedReviewer: null,
				commitSha: "abc",
				commitSummary: "wip",
				commitAuthorName: "carol",
				createdAt: "2026-08-28T11:40:00Z",
			},
		];
		const items = activityItems([], submissions, events, 0);
		expect(items.map((item) => item.kind)).toEqual(["verdict", "reviewRequested", "committed"]);
	});
});

describe("itemMentions", () => {
	it("matches a mention of the login, case-insensitively", () => {
		expect(itemMentions(comment("alice", 1, "ping @Me please"), "me")).toBe(true);
		expect(itemMentions(verdict("alice", "commented", 1, "@me look"), "me")).toBe(true);
	});

	it("requires the whole login, not a prefix of a longer one", () => {
		expect(itemMentions(comment("alice", 1, "cc @me-too"), "me")).toBe(false);
		expect(itemMentions(comment("alice", 1, "mail me@example.com"), "me")).toBe(false);
	});

	it("never matches without a login or a body", () => {
		expect(itemMentions(comment("alice", 1, "@me"), null)).toBe(false);
		expect(itemMentions({ kind: "committed", author: "alice", atMs: 1 }, "me")).toBe(false);
	});
});

describe("mentions", () => {
	it("makes even a dismissed verdict loud", () => {
		expect(attentionOf(verdict("alice", "dismissed", 1, "sorry @me, superseded"), "me")).toBe(
			"loud",
		);
	});

	it("stays silent for a mention in the user's own text", () => {
		expect(attentionOf(comment("me", 1, "note to @me"), "me")).toBe("silent");
	});
});
