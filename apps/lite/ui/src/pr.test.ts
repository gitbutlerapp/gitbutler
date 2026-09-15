import { describe, expect, it } from "vitest";
import { mergeReadiness, reviewBodyVerdict, reviewReadinessVerdicts } from "./pr.ts";

describe("merge readiness", () => {
	const review = { draft: false, mergedAt: null, closedAt: null, reviewers: [] };
	it("leaves merge authorization with the forge even with requested reviewers", () => {
		const ready = mergeReadiness(
			{ ...review, reviewers: [{}] },
			{ isMergeable: true, mergeableState: "clean" },
			"success",
			["approved"],
		);
		expect(ready.ready).toBe(true);
		expect(ready.blocker).toBeNull();
		expect(ready.rows.map((row) => row.label)).toContain("1 review pending");
	});
	it("names formal change requests as blockers and hides unreported conflicts", () => {
		const result = mergeReadiness(
			{ ...review, reviewers: [{}, {}, {}, {}] },
			{ isMergeable: false, mergeableState: "blocked" },
			"success",
			["changesRequested"],
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
	it("keeps recommendations out of the merge blocker", () => {
		const result = mergeReadiness(
			review,
			{ isMergeable: false, mergeableState: "blocked" },
			"success",
			["changesRecommended", "needsCloserLook"],
		);
		expect(result.rows[1]).toEqual({ label: "Changes recommended", tone: "warn", clear: false });
		expect(result.blocker).toBe("Blocked: required approvals or checks are not satisfied");
		expect(result.headline).toBe("Merge blocked");
	});
	it("counts each clear condition, including zero pending reviews with a hollow dot", () => {
		const result = mergeReadiness(
			review,
			{ isMergeable: true, mergeableState: "clean" },
			"success",
			["approved"],
		);
		expect(result.clearCount).toBe(4);
		expect(result.rows[2]).toEqual({ label: "0 reviews pending", tone: "pending", clear: true });
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
		expect(result.clearCount).toBe(2);
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
		const labels = (checks: "unknown" | undefined) =>
			mergeReadiness({ ...review, draft: true }, undefined, checks, []).rows.map(
				(row) => row.label,
			);
		expect(labels(undefined)).toEqual(["No review verdict", "0 reviews pending"]);
		expect(labels("unknown")).toEqual(["No review verdict", "0 reviews pending"]);
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
	])("extracts the leading verdict from %s without changing the prose", (source, verdict) => {
		expect(reviewBodyVerdict(source)).toEqual({ body: "Fix this.", verdict });
	});
	it.each([
		"Approved by someone else, but I have questions.",
		"> 🟡 Changes recommended\nQuoted feedback.",
		"Introduction\n## 🟡 Changes recommended\nDetails.",
		"```\nApproved\n```",
	])("keeps ordinary prose and non-leading headings: %s", (body) => {
		expect(reviewBodyVerdict(body)).toEqual({ body });
	});
});

describe("readiness from review activity", () => {
	const author = { id: 1, login: "agent", name: null, email: null, avatarUrl: null, isBot: true };
	it("includes ordinary comment verdicts and lets a later formal approval supersede them", () => {
		const comments = [
			{
				id: 1,
				author,
				body: "## 🟡 Changes recommended\nFix this.",
				createdAt: "2026-09-01T00:00:00Z",
				modifiedAt: null,
			},
		];
		expect(reviewReadinessVerdicts([], comments)).toEqual(["changesRecommended"]);
		expect(
			reviewReadinessVerdicts(
				[{ id: 2, author, state: "approved", body: null, submittedAt: "2026-09-02T00:00:00Z" }],
				comments,
			),
		).toEqual(["approved"]);
	});
	it("keeps only the latest heading per author and ignores ordinary prose", () => {
		expect(
			reviewReadinessVerdicts(
				[],
				[
					{
						id: 2,
						author,
						body: "🔵 Needs a closer look\nDetails",
						createdAt: "2026-09-02T00:00:00Z",
						modifiedAt: null,
					},
					{
						id: 1,
						author,
						body: "🟡 Changes recommended\nDetails",
						createdAt: "2026-09-01T00:00:00Z",
						modifiedAt: null,
					},
					{
						id: 3,
						author,
						body: "Thanks for the update.",
						createdAt: "2026-09-03T00:00:00Z",
						modifiedAt: null,
					},
				],
			),
		).toEqual(["needsCloserLook"]);
	});
	it("clears a review's advice once every thread it opened is resolved", () => {
		const advice = {
			id: 7,
			author,
			state: "commented" as const,
			body: "### 🟡 Changes recommended\nFix this.",
			submittedAt: "2026-09-01T00:00:00Z",
		};
		const thread = (reviewId: number, isResolved: boolean) => ({
			isResolved,
			comments: [{ reviewId }],
		});
		expect(reviewReadinessVerdicts([advice], [], [thread(7, true), thread(7, false)])).toEqual([
			"changesRecommended",
		]);
		expect(reviewReadinessVerdicts([advice], [], [thread(7, true), thread(7, true)])).toEqual([
			"commented",
		]);
		// Nothing to resolve, and a thread under another review, both leave the advice standing.
		expect(reviewReadinessVerdicts([advice], [], [])).toEqual(["changesRecommended"]);
		expect(reviewReadinessVerdicts([advice], [], [thread(8, true)])).toEqual([
			"changesRecommended",
		]);
		// Only the forge can lift a formal change request.
		const formal = { ...advice, state: "changesRequested" as const, body: null };
		expect(reviewReadinessVerdicts([formal], [], [thread(7, true)])).toEqual(["changesRequested"]);
	});
	it("lets a recommended approval supersede earlier advice", () => {
		const later = {
			id: 8,
			author,
			state: "commented" as const,
			body: "### 🟢 Approval recommended\nLooks good now.",
			submittedAt: "2026-09-02T00:00:00Z",
		};
		const earlier = {
			...later,
			id: 7,
			body: "### 🟡 Changes recommended\nFix this.",
			submittedAt: "2026-09-01T00:00:00Z",
		};
		expect(reviewReadinessVerdicts([earlier, later], [])).toEqual(["approvalRecommended"]);
		const row = mergeReadiness(
			{ draft: false, mergedAt: null, closedAt: null, reviewers: [] },
			{ isMergeable: true, mergeableState: "clean" },
			"success",
			["approvalRecommended"],
		).rows[1];
		expect(row).toEqual({ label: "Approval recommended", tone: "safe", clear: true });
	});
	it("treats a bot's REST and GraphQL logins as one reviewer", () => {
		// Comments come over REST with the `[bot]` suffix; submissions over GraphQL without it.
		expect(
			reviewReadinessVerdicts(
				[
					{
						id: 2,
						author: { ...author, login: "copilot-pull-request-reviewer" },
						state: "approved",
						body: null,
						submittedAt: "2026-09-02T00:00:00Z",
					},
				],
				[
					{
						id: 1,
						author: { ...author, login: "copilot-pull-request-reviewer[bot]" },
						body: "🟡 Changes recommended\nFix this.",
						createdAt: "2026-09-01T00:00:00Z",
						modifiedAt: null,
					},
				],
			),
		).toEqual(["approved"]);
	});
});
