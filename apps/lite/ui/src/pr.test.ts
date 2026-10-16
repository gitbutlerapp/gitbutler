import { describe, expect, it } from "vitest";
import { mergeReadiness, reviewBodyVerdict, reviewerRows } from "./pr.ts";
import type { ForgeReviewSubmission } from "@gitbutler/but-sdk";

describe("merge readiness", () => {
	const review = { draft: false, mergedAt: null, closedAt: null };
	it("leaves merge authorization with the forge even with requested reviewers", () => {
		const ready = mergeReadiness(
			review,
			{ isMergeable: true, mergeableState: "clean" },
			"success",
			["approved", "awaiting"],
		);
		expect(ready.ready).toBe(true);
		expect(ready.blocker).toBeNull();
		expect(ready.rows.map((row) => row.label)).toContain("1 review pending");
	});
	it("names formal change requests as blockers and hides unreported conflicts", () => {
		const result = mergeReadiness(
			review,
			{ isMergeable: false, mergeableState: "blocked" },
			"success",
			["changesRequested", "awaiting", "awaiting", "awaiting", "awaiting"],
		);
		expect(result.headline).toBe("Blocked on review");
		expect(result.tone).toBe("danger");
		expect(result.blocker).toBe("Changes requested · 4 reviews pending");
		expect(result.rows.map((row) => row.label)).toEqual([
			"Checks passed",
			"Changes requested",
			"4 reviews pending",
		]);
		expect(result.clearCount).toBe(1);
	});
	it("reports open conversations without naming them as the merge blocker", () => {
		const result = mergeReadiness(
			review,
			{ isMergeable: false, mergeableState: "blocked" },
			"success",
			["commented"],
			[{ isResolved: false }, { isResolved: true }, { isResolved: false }],
		);
		expect(result.rows[1]).toEqual({
			label: "2 unresolved conversations",
			tone: "warn",
			clear: false,
		});
		expect(result.blocker).toBe("Blocked: required approvals or checks are not satisfied");
		expect(result.headline).toBe("Merge blocked");
	});
	it("clears the conversations once every one is resolved", () => {
		const result = mergeReadiness(
			review,
			{ isMergeable: true, mergeableState: "clean" },
			"success",
			["commented"],
			[{ isResolved: true }, { isResolved: true }],
		);
		expect(result.rows[1]).toEqual({ label: "Conversations resolved", tone: "safe", clear: true });
		expect(result.clearCount).toBe(3);
	});
	it("counts each clear condition, and leaves out reviews nobody is waiting on", () => {
		const result = mergeReadiness(
			review,
			{ isMergeable: true, mergeableState: "clean" },
			"success",
			["approved"],
		);
		expect(result.rows.map((row) => row.label)).toEqual([
			"Checks passed",
			"Approved",
			"No conflicts",
		]);
		expect(result.clearCount).toBe(3);
		expect(result.tone).toBe("safe");
	});
	it("names failing checks and gives the headline a danger tone", () => {
		const result = mergeReadiness(
			review,
			{ isMergeable: false, mergeableState: "blocked" },
			"failure",
			["approved"],
		);
		expect(result.headline).toBe("Checks failing");
		expect(result.tone).toBe("danger");
		expect(result.clearCount).toBe(1);
	});
	it("calls cancelled checks cancelled, not failed", () => {
		const result = mergeReadiness(
			review,
			{ isMergeable: false, mergeableState: "blocked" },
			"cancelled",
			["approved"],
		);
		expect(result.rows[0]).toEqual({ label: "Checks cancelled", tone: "warn", clear: false });
		expect(result.headline).toBe("Blocked on checks");
	});
	it.each(["unstable", "has_hooks"] as const)("reads %s as free of conflicts", (mergeableState) => {
		const result = mergeReadiness(review, { isMergeable: false, mergeableState }, "success", [
			"approved",
		]);
		expect(result.rows.at(-1)).toEqual({ label: "No conflicts", tone: "safe", clear: true });
	});
	it("shows only the rows that have something to report", () => {
		const labels = (checks: null | undefined) =>
			mergeReadiness({ ...review, draft: true }, undefined, checks, []).rows.map(
				(row) => row.label,
			);
		expect(labels(undefined)).toEqual([]);
		expect(labels(null)).toEqual([]);
	});
	it("reports checks that exist but have not resolved as pending", () => {
		const result = mergeReadiness(
			review,
			{ isMergeable: false, mergeableState: "blocked" },
			"unknown",
			["approved"],
		);
		expect(result.rows[0]).toEqual({ label: "Checks pending", tone: "warn", clear: false });
		expect(result.headline).toBe("Blocked on checks");
	});
	it("adds no headline when the status badge already says where the PR stands", () => {
		const settled = "2026-09-01T00:00:00Z";
		expect(
			mergeReadiness({ ...review, draft: true }, undefined, undefined, []).headline,
		).toBeNull();
		expect(
			mergeReadiness({ ...review, mergedAt: settled }, undefined, undefined, []).headline,
		).toBeNull();
		expect(
			mergeReadiness({ ...review, closedAt: settled }, undefined, undefined, []).headline,
		).toBeNull();
	});
	it("reports unknown, conflicts, draft, and additional backend restrictions truthfully", () => {
		expect(mergeReadiness(review, undefined, undefined, []).ready).toBe(false);
		expect(mergeReadiness(review, undefined, undefined, []).blocker).toBe("Checking mergeability…");
		expect(
			mergeReadiness(review, { isMergeable: false, mergeableState: "dirty" }, "success", [])
				.blocker,
		).toBe("Merge conflicts with the base branch");
		expect(mergeReadiness({ ...review, draft: true }, undefined, undefined, []).blocker).toBe(
			"Draft pull requests cannot be merged",
		);
		expect(
			mergeReadiness(review, { isMergeable: false, mergeableState: "behind" }, "success", [])
				.blocker,
		).toBe("Behind the base branch; update the branch first");
	});
});

describe("review verdict bylines", () => {
	it.each([
		["## 🟡 Changes recommended\n\nFix this.", "changesRecommended"],
		["🔵 Needs a closer look\r\n\r\nFix this.", "needsCloserLook"],
		["### 🟢 Approval recommended\n\nFix this.", "approvalRecommended"],
		["### **Approved**\n\nFix this.", "approved"],
		[
			"<!-- ccr-overview-v2 -->\n\n## Copilot review overview\n\n### 🟡 Changes recommended\n\nFix this.",
			"changesRecommended",
		],
	])("extracts the leading verdict from %s without changing the prose", (source, verdict) => {
		expect(reviewBodyVerdict(source)).toEqual({ body: "Fix this.", verdict });
	});
	it.each([
		"Approved by someone else, but I have questions.",
		"> 🟡 Changes recommended\nQuoted feedback.",
		"Introduction\n## 🟡 Changes recommended\nDetails.",
		"## Overview\n\nSome prose first.\n\n### 🟡 Changes recommended\nDetails.",
		"```\nApproved\n```",
	])("keeps ordinary prose and non-leading headings: %s", (body) => {
		expect(reviewBodyVerdict(body)).toEqual({ body });
	});
});

describe("reviewer standing", () => {
	const user = (login: string) => ({
		id: 1,
		login,
		name: null,
		email: null,
		avatarUrl: null,
		isBot: true,
	});
	const submission = (
		id: number,
		login: string,
		state: ForgeReviewSubmission["state"],
	): ForgeReviewSubmission => ({
		id,
		author: user(login),
		state,
		body: null,
		submittedAt: null,
		htmlUrl: "",
		reactions: [],
	});
	const verdicts = (...args: Parameters<typeof reviewerRows>) =>
		reviewerRows(...args).map((row) => row.verdict);

	it("keeps a verdict the forge holds against a later comment, until the forge dismisses it", () => {
		const held = [submission(1, "alice", "changesRequested"), submission(2, "alice", "commented")];
		expect(verdicts([], held)).toEqual(["changesRequested"]);
		expect(verdicts([], [...held, submission(3, "alice", "dismissed")])).toEqual(["commented"]);
	});
	it("counts a reviewer as awaiting only until they answer, under either spelling of a bot", () => {
		const requested = [user("review-agent[bot]"), user("bob")];
		expect(verdicts(requested, [])).toEqual(["awaiting", "awaiting"]);
		expect(verdicts(requested, [submission(1, "review-agent", "commented")])).toEqual([
			"commented",
			"awaiting",
		]);
	});
});
