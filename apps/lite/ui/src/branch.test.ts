import { describe, expect, it } from "vitest";

import {
	activeBranchFilterCount,
	branchDetailsParams,
	branchIsEmpty,
	branchOwnCommits,
	searchStacks,
	unappliedStacks,
} from "./branch.ts";
import type { ListedBranch, ListedStack } from "@gitbutler/but-sdk";

const branch = (overrides: Partial<ListedBranch> & { displayName: string }): ListedBranch => ({
	refName: { full: `refs/heads/${overrides.displayName}` },
	tip: "0".repeat(40),
	hasLocal: true,
	remoteRefs: [],
	commitCount: 1,
	commitsAheadOfTarget: 1,
	lastAuthor: null,
	updatedAtMs: null,
	review: null,
	reviewStatus: null,
	...overrides,
});

const stack = (
	branches: Array<ListedBranch>,
	overrides: Partial<ListedStack> = {},
): ListedStack => ({
	status: "standalone",
	branches,
	updatedAtMs: null,
	...overrides,
});

const names = (stacks: Array<ListedStack>): Array<Array<string>> =>
	stacks.map((s) => s.branches.map((b) => b.displayName));

const allFilters = { showEmpty: false, onlyLocal: false, onlyStacks: false };

describe("activeBranchFilterCount", () => {
	it("counts the options switched on", () => {
		expect(activeBranchFilterCount(allFilters)).toBe(0);
		expect(activeBranchFilterCount({ ...allFilters, onlyLocal: true })).toBe(1);
		expect(activeBranchFilterCount({ showEmpty: true, onlyLocal: true, onlyStacks: true })).toBe(3);
	});
});

describe("branchIsEmpty", () => {
	it("is empty only at exactly zero commits", () => {
		expect(branchIsEmpty(branch({ displayName: "a", commitCount: 0 }))).toBe(true);
		expect(branchIsEmpty(branch({ displayName: "a", commitCount: 3 }))).toBe(false);
	});

	it("treats an unknown count as not empty, since it may hold commits", () => {
		expect(branchIsEmpty(branch({ displayName: "a", commitCount: null }))).toBe(false);
	});
});

describe("branchOwnCommits", () => {
	// Branch details are tip-first and run past this branch into the ones below.
	const commits = ["tip", "middle", "below-1", "below-2"];

	it("takes the branch's own commits off the tip", () => {
		expect(branchOwnCommits(branch({ displayName: "a", commitCount: 2 }), commits)).toEqual([
			"tip",
			"middle",
		]);
	});

	it("keeps everything when the count is unknown", () => {
		expect(branchOwnCommits(branch({ displayName: "a", commitCount: null }), commits)).toEqual(
			commits,
		);
	});

	it("takes nothing from an empty branch", () => {
		expect(branchOwnCommits(branch({ displayName: "a", commitCount: 0 }), commits)).toEqual([]);
	});
});

describe("unappliedStacks", () => {
	it("keeps unapplied and standalone stacks, dropping applied and target ones", () => {
		const stacks = [
			stack([branch({ displayName: "applied" })], { status: "applied" }),
			stack([branch({ displayName: "unapplied" })], { status: "unapplied" }),
			stack([branch({ displayName: "standalone" })], { status: "standalone" }),
			stack([branch({ displayName: "target" })], { status: "target" }),
		];

		expect(names(unappliedStacks(stacks, allFilters))).toEqual([["unapplied"], ["standalone"]]);
	});

	it("drops empty branches, and stacks left with nothing", () => {
		const stacks = [
			stack([branch({ displayName: "top" }), branch({ displayName: "empty", commitCount: 0 })]),
			stack([branch({ displayName: "all-empty", commitCount: 0 })]),
		];

		expect(names(unappliedStacks(stacks, allFilters))).toEqual([["top"]]);
	});

	it("keeps empty branches when showEmpty is set", () => {
		const stacks = [stack([branch({ displayName: "empty", commitCount: 0 })])];

		expect(names(unappliedStacks(stacks, { ...allFilters, showEmpty: true }))).toEqual([["empty"]]);
	});

	it("drops remote-only branches when onlyLocal is set", () => {
		const stacks = [
			stack([
				branch({ displayName: "local" }),
				branch({ displayName: "remote-only", hasLocal: false }),
			]),
		];

		expect(names(unappliedStacks(stacks, { ...allFilters, onlyLocal: true }))).toEqual([["local"]]);
	});

	it("keeps only multi-branch stacks when onlyStacks is set", () => {
		const stacks = [
			stack([branch({ displayName: "top" }), branch({ displayName: "bottom" })]),
			stack([branch({ displayName: "lone" })]),
		];

		expect(names(unappliedStacks(stacks, { ...allFilters, onlyStacks: true }))).toEqual([
			["top", "bottom"],
		]);
	});

	it("applies onlyStacks after empty branches are dropped", () => {
		// Only multi-branch because of an empty branch, so it is not a stack once
		// that branch is filtered out.
		const stacks = [
			stack([branch({ displayName: "top" }), branch({ displayName: "empty", commitCount: 0 })]),
		];

		expect(names(unappliedStacks(stacks, { ...allFilters, onlyStacks: true }))).toEqual([]);
		expect(
			names(unappliedStacks(stacks, { ...allFilters, onlyStacks: true, showEmpty: true })),
		).toEqual([["top", "empty"]]);
	});
});

describe("searchStacks", () => {
	const stacks = [
		stack([branch({ displayName: "feature-login", lastAuthor: null })]),
		stack([
			branch({
				displayName: "chore-deps",
				lastAuthor: { name: "Ada Lovelace", email: "ada@example.com", gravatarUrl: "" },
			}),
		]),
		stack([
			branch({
				displayName: "unrelated",
				review: {
					number: 42,
					title: "Speed up the parser",
					htmlUrl: "https://example.com/42",
					unitSymbol: "#",
					labels: [{ name: "performance", color: "00ff00", description: null }],
					author: { login: "octocat", name: "Grace Hopper" },
					createdAt: null,
				},
			}),
		]),
	];

	it("does not filter on queries below the minimum length", () => {
		expect(names(searchStacks(stacks, "f"))).toEqual(names(stacks));
		expect(names(searchStacks(stacks, "  "))).toEqual(names(stacks));
	});

	it("matches on branch name", () => {
		expect(names(searchStacks(stacks, "login"))).toEqual([["feature-login"]]);
	});

	it("matches on author name and email", () => {
		expect(names(searchStacks(stacks, "Lovelace"))).toEqual([["chore-deps"]]);
		expect(names(searchStacks(stacks, "ada@example"))).toEqual([["chore-deps"]]);
	});

	it("matches on review title", () => {
		expect(names(searchStacks(stacks, "parser"))).toEqual([["unrelated"]]);
	});

	it("matches labels and the review author independently of the last commit author", () => {
		expect(names(searchStacks(stacks, "performance"))).toEqual([["unrelated"]]);
		expect(names(searchStacks(stacks, "octocat"))).toEqual([["unrelated"]]);
		expect(names(searchStacks(stacks, "Hopper"))).toEqual([["unrelated"]]);
	});

	it("matches exact review numbers with a forge symbol", () => {
		expect(names(searchStacks(stacks, "#42"))).toEqual([["unrelated"]]);
		expect(searchStacks(stacks, "#17")).toEqual([]);
		expect(searchStacks(stacks, "!42")).toEqual([]);
	});

	it("still searches branch names when a number has no forge symbol", () => {
		const withNumberedBranch = [...stacks, stack([branch({ displayName: "release-42" })])];
		expect(names(searchStacks(withNumberedBranch, "42"))).toEqual([["unrelated"], ["release-42"]]);
	});

	it("keeps a matched stack whole, including its non-matching branches", () => {
		const stacked = [
			stack([branch({ displayName: "feature-top" }), branch({ displayName: "zzz-bottom" })]),
		];

		expect(names(searchStacks(stacked, "feature-top"))).toEqual([["feature-top", "zzz-bottom"]]);
	});

	it("returns nothing when no branch matches", () => {
		expect(searchStacks(stacks, "nonexistent-branch-name")).toEqual([]);
	});
});

describe("branchDetailsParams", () => {
	it("strips the local ref prefix and reports no remote", () => {
		expect(branchDetailsParams("refs/heads/feature/login")).toEqual({
			branchName: "feature/login",
			remote: null,
		});
	});

	it("splits a remote-tracking ref into remote and branch name", () => {
		expect(branchDetailsParams("refs/remotes/origin/feature/login")).toEqual({
			branchName: "feature/login",
			remote: "origin",
		});
	});

	it("leaves a bare name untouched", () => {
		expect(branchDetailsParams("feature/login")).toEqual({
			branchName: "feature/login",
			remote: null,
		});
	});
});
