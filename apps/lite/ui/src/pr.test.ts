import { describe, expect, it } from "vitest";
import { mergeReadiness, reviewVerdictBody, reviewReadinessVerdicts } from "./pr.ts";

describe("review verdict headings", () => {
	it("moves a known leading heading into metadata while preserving prose", () => {
		expect(reviewVerdictBody("## 🟡 Changes recommended\n\nKeep this prose.")).toEqual({
			body: "Keep this prose.",
			badge: { label: "Changes recommended", variant: "warn" },
		});
		expect(reviewVerdictBody("🔵 Needs a closer look\nDetails").badge?.variant).toBe("pop");
		expect(reviewVerdictBody("### ✅ Approved\nLooks good").badge?.variant).toBe("safe");
	});
	it("preserves unrecognized and non-leading headings", () => {
		const body = "Intro\n## 🟡 Changes recommended\nDetails";
		expect(reviewVerdictBody(body)).toEqual({ body, badge: undefined });
		expect(reviewVerdictBody("## Other verdict\nDetails").body).toBe("## Other verdict\nDetails");
	});
});

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
	it("names review blockers and does not infer clean conflicts from blocked status", () => {
		const result = mergeReadiness(
			{ ...review, reviewers: [{}, {}, {}, {}] },
			{ isMergeable: false, mergeableState: "blocked" },
			"success",
			["changesRequested"],
		);
		expect(result.headline).toBe("Blocked on review");
		expect(result.blocker).toBe("Changes recommended · 4 reviews pending");
		expect(result.rows.at(-1)).toEqual({ label: "Conflicts not reported", tone: "pending" });
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
		expect(reviewReadinessVerdicts([], comments)).toEqual(["changesRequested"]);
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
});
