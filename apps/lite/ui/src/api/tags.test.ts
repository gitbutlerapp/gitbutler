import { invalidateDeclared, invalidateTags } from "#ui/api/tags.ts";
import { apiInvalidates, type CacheTag } from "@gitbutler/but-sdk/cache-tags";
import type { QueryClient, QueryFilters } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

const declared: Record<string, ReadonlyArray<CacheTag>> = apiInvalidates;

const recording = () => {
	const invalidated: Array<ReadonlyArray<unknown>> = [];
	const predicates: Array<NonNullable<QueryFilters["predicate"]>> = [];
	const client: Pick<QueryClient, "invalidateQueries"> = {
		invalidateQueries: (filters) => {
			if (filters?.queryKey) invalidated.push(filters.queryKey);
			if (filters?.predicate) predicates.push(filters.predicate);
			return Promise.resolve();
		},
	};
	return { client, invalidated, predicates };
};

describe("invalidateTags", () => {
	it("scopes project queries to the project", async () => {
		const { client, invalidated } = recording();
		await invalidateTags(client, ["Reviews"], "p1");
		expect(invalidated).toEqual(
			expect.arrayContaining([
				["p1", "getReview"],
				["p1", "listReviews"],
			]),
		);
	});

	it("matches the endpoint across projects without a project id", async () => {
		const { client, invalidated, predicates } = recording();
		await invalidateTags(client, ["ForgeLogin"]);
		expect(invalidated).toEqual([]);
		expect(predicates).toHaveLength(1);
		expect(predicates[0]?.({ queryKey: ["p1", "currentForgeLogin"] } as never)).toBe(true);
		expect(predicates[0]?.({ queryKey: ["p1", "headInfo"] } as never)).toBe(false);
	});

	it("reaches global queries", async () => {
		const { client, invalidated } = recording();
		await invalidateTags(client, ["Projects", "ForgeAccounts"], "p1");
		expect(invalidated).toEqual(expect.arrayContaining([["projects"], ["forgeAccounts"]]));
	});
});

describe("declared mutations", () => {
	// A tag no query provides invalidates nothing: the declaration is dead
	// and the mutation author believes otherwise.
	it.each(Object.entries(declared))(
		"%s only names tags some query provides",
		async (_endpoint, tags) => {
			const { client, invalidated } = recording();
			await invalidateTags(client, tags, "p1");
			expect(invalidated.length).toBeGreaterThan(0);
		},
	);

	it("applies a project mutation's declaration from its key", async () => {
		const { client, invalidated } = recording();
		await invalidateDeclared(client, ["p1", "mergeReview"]);
		expect(invalidated).toEqual(
			expect.arrayContaining([
				["p1", "getReview"],
				["p1", "listReviews"],
				["p1", "getReviewMergeStatus"],
				["p1", "listCiChecks"],
			]),
		);
	});

	it("applies a global mutation's declaration from its key", async () => {
		const { client, invalidated, predicates } = recording();
		await invalidateDeclared(client, ["storeGithubPat"]);
		expect(invalidated).toContainEqual(["forgeAccounts"]);
		expect(predicates).toHaveLength(1);
		expect(predicates[0]?.({ queryKey: ["p1", "currentForgeLogin"] } as never)).toBe(true);
	});

	it("ignores mutations that declared nothing", async () => {
		const { client, invalidated } = recording();
		await invalidateDeclared(client, ["p1", "commitAmend"]);
		await invalidateDeclared(client, undefined);
		expect(invalidated).toEqual([]);
	});
});
