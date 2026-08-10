import { invalidateDeclared, invalidateTags } from "#ui/api/tags.ts";
import { apiInvalidates, type CacheTag } from "@gitbutler/but-sdk/cache-tags";
import type { QueryClient } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

const declared = apiInvalidates as Record<string, ReadonlyArray<CacheTag>>;

const recording = () => {
	const invalidated: Array<ReadonlyArray<unknown>> = [];
	const client = {
		invalidateQueries: ({ queryKey }: { queryKey: ReadonlyArray<unknown> }) => {
			invalidated.push(queryKey);
			return Promise.resolve();
		},
	} as unknown as QueryClient;
	return { client, invalidated };
};

describe("invalidateTags", () => {
	it("scopes project queries to the project", async () => {
		const { client, invalidated } = recording();
		await invalidateTags(client, ["Reviews"], "p1");
		expect(invalidated).toEqual(
			expect.arrayContaining([
				["getReview", "p1"],
				["listReviews", "p1"],
			]),
		);
	});

	it("falls back to a key prefix without a project id", async () => {
		const { client, invalidated } = recording();
		await invalidateTags(client, ["ForgeLogin"]);
		expect(invalidated).toEqual([["currentForgeLogin"]]);
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

	it("applies a mutation's declaration from its key", async () => {
		const { client, invalidated } = recording();
		await invalidateDeclared(client, ["mergeReview"], { projectId: "p1" });
		expect(invalidated).toEqual(
			expect.arrayContaining([
				["getReview", "p1"],
				["listReviews", "p1"],
				["getReviewMergeStatus", "p1"],
				["listCiChecks", "p1"],
			]),
		);
	});

	it("ignores mutations that declared nothing", async () => {
		const { client, invalidated } = recording();
		await invalidateDeclared(client, ["commitCreate"], { projectId: "p1" });
		await invalidateDeclared(client, undefined, { projectId: "p1" });
		expect(invalidated).toEqual([]);
	});
});
