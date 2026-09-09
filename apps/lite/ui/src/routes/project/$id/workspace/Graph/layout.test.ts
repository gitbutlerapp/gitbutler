import type {
	Commit,
	Stack,
	Target,
	TargetCommit,
	TargetCommitPage,
	Worktree,
} from "@gitbutler/but-sdk";
import { describe, expect, it } from "vitest";
import type { Address } from "#ui/addresses.ts";
import {
	inSection,
	layout,
	sectionAddresses,
	targetCommitAddress,
	worktreesOnTip,
} from "./layout.ts";

type Folds = Parameters<typeof layout>[3];

/** How much of a long run shows at first, as layout.ts has it. */
const FIRST = 10;

const commit = (id: string, inWorkspace: boolean): TargetCommit => ({
	commit: {
		id,
		message: id,
		authoredAt: 0,
		committedAt: 0,
		author: { name: "", email: "", gravatarUrl: "" },
		changeId: null,
	},
	review: null,
	inWorkspace,
});

const stack = (base: string | null): Stack => ({ id: null, base, segments: [] });

const ownCommit = (id: string): Commit => ({
	id,
	parentIds: [],
	message: id,
	hasConflicts: false,
	state: { type: "LocalOnly" },
	authoredAt: 0,
	committedAt: 0,
	author: { name: "", email: "", gravatarUrl: "" },
	changeId: id,
	gerritReviewUrl: null,
});

/** A stack of one segment holding `commits`, named `branch` if given. */
const stackWith = (base: string, commits: Array<string>, branch?: string): Stack => ({
	id: null,
	base,
	segments: [
		{
			refName: branch === undefined ? null : { fullNameBytes: [], displayName: branch },
			remoteTrackingRefName: null,
			commits: commits.map(ownCommit),
			commitsOnRemote: [],
			metadata: null,
			pushStatus: "nothingToPush",
			base,
		},
	],
});

const worktree = (name: string, base: Worktree["base"], commits: Array<string>): Worktree => ({
	name,
	refName: null,
	head: commits[0] ?? "",
	base,
	segments: [
		{
			refName: null,
			remoteTrackingRefName: null,
			commits: commits.map(ownCommit),
			commitsOnRemote: [],
			metadata: null,
			pushStatus: "completelyUnpushed",
			base: base?.subject ?? null,
		},
	],
});

const target = (isCurrent: boolean): Target => ({
	remoteTrackingRef: { fullNameBytes: [], displayName: "master", remoteName: "origin" },
	commitsAhead: 0,
	isCurrent,
});

const folded: Folds = { incomingExpanded: false, moreRuns: {} };
const expanded: Folds = { ...folded, incomingExpanded: true };

// Newest first: two incoming, a shallow base, three had, a deep base.
const listing: TargetCommitPage = {
	commits: [
		commit("i1", false),
		commit("i2", false),
		commit("shallow", true),
		commit("h1", true),
		commit("h2", true),
		commit("h3", true),
		commit("deep", true),
	],
	hasMore: false,
};

/** A page of incoming commits above one base. */
const incomingAbove = (ids: Array<string>): TargetCommitPage => ({
	commits: [...ids.map((id) => commit(id, false)), commit("base", true)],
	hasMore: false,
});

describe("layout", () => {
	it("orders cards by base depth, deepest first, then as given", () => {
		const plan = layout([stack("deep"), stack("shallow"), stack("deep")], null, listing, folded);
		expect(plan.order).toEqual([0, 2, 1]);
		// A base the listing does not reach is the deepest; several keep their order.
		expect(layout([stack("shallow"), stack("unlisted")], null, listing, folded).order).toEqual([
			1, 0,
		]);
		expect(
			layout([stack("shallow"), stack("unlisted"), stack("another")], null, listing, folded).order,
		).toEqual([1, 2, 0]);
	});

	it("folds the incoming leg under the target's header", () => {
		const plan = layout([stack("deep")], target(true), listing, folded);
		expect(plan.incoming).toEqual([]);
		expect(plan.header?.incoming).toBe(2);
	});

	it("puts the incoming commits on their own leg", () => {
		const plan = layout([stack("deep"), stack("shallow")], null, listing, expanded);
		expect(plan.incoming.map((run) => run.shown.map((c) => c.commit.id))).toEqual([["i1", "i2"]]);
	});

	it("lists the section's rows as values and knows which sit in it", () => {
		const plan = layout([stack("deep"), stack("shallow")], null, listing, expanded);
		const ids = (addresses: Array<Address>) =>
			addresses.map((address) => (address._tag === "Commit" ? address.commitId : address._tag));
		// The incoming commits only: the workspace's own commits and the header are not values.
		expect(ids(sectionAddresses(plan))).toEqual(["i1", "i2"]);
		const at = (id: string) => inSection(plan, targetCommitAddress(commit(id, true)));
		expect(at("i1")).toBe(true);
		expect(at("deep")).toBe(false);
		expect(at("h1")).toBe(false);
	});

	it("shows an incoming run's newest first, more with each ask, unless a fold would hide one row", () => {
		const ids = Array.from({ length: 25 }, (_, i) => `c${i}`);
		const many = incomingAbove(ids);
		const run = layout([stack("base")], null, many, expanded).incoming[0];
		expect(run?.shown.length).toBe(FIRST);
		expect(run?.hidden).toBe(25 - FIRST);
		expect(run?.expanded).toBe(false);
		const asked = { ...expanded, moreRuns: { c0: 1 } };
		const more = layout([stack("base")], null, many, asked).incoming[0];
		expect(more?.shown.length).toBe(25);
		expect(more?.hidden).toBe(0);
		expect(more?.expanded).toBe(true);

		const eleven = incomingAbove(ids.slice(0, FIRST + 1));
		const whole = layout([stack("base")], null, eleven, expanded).incoming[0];
		expect(whole?.shown.length).toBe(FIRST + 1);
		expect(whole?.hidden).toBe(0);
	});

	it("names the target's row once its commits are known, and says whether the base is at its tip", () => {
		expect(layout([], target(true), undefined, folded).header).toBeNull();
		expect(layout([], null, listing, folded).header).toBeNull();
		expect(layout([], target(true), listing, folded).header).toEqual({
			label: "origin/master",
			incoming: 2,
			current: true,
		});
		expect(layout([], target(false), listing, folded).header?.current).toBe(false);
	});

	it("nests a worktree above the shown commit it rests on, and stands the rest alone", () => {
		const inside = worktree("inside", { type: "InWorkspace", subject: "a1" }, ["w1"]);
		const onWorktree = worktree("stacked", { type: "InWorkspace", subject: "w1" }, ["w2"]);
		const below = worktree("below", { type: "Outside", subject: "deep" }, ["o1"]);
		const unknown = worktree("unknown", null, []);
		const { worktrees } = layout([stackWith("shallow", ["a1", "a2"])], null, listing, folded, [
			inside,
			onWorktree,
			below,
			unknown,
		]);
		expect(worktrees.on.get("a1")).toEqual([inside]);
		// A worktree's own commits count as shown, so one resting on them nests inside its lane.
		expect(worktrees.on.get("w1")).toEqual([onWorktree]);
		expect(worktrees.standalone).toEqual([below, unknown]);
	});

	it("lifts a worktree on the top branch's tip above that branch, and no other", () => {
		const onTip = worktree("tip", { type: "InWorkspace", subject: "a1" }, ["w1"]);
		const below = worktree("below", { type: "InWorkspace", subject: "a2" }, ["w2"]);
		const named = stackWith("shallow", ["a1", "a2"], "A");
		const { worktrees } = layout([named], null, listing, folded, [onTip, below]);
		expect(worktreesOnTip(worktrees, named)).toEqual([onTip]);
		// An unnamed top segment has no branch row to sit above.
		expect(worktreesOnTip(worktrees, stackWith("shallow", ["a1", "a2"]))).toEqual([]);
	});

	it("keeps worktrees resting on one commit in the order given", () => {
		const first = worktree("first", { type: "InWorkspace", subject: "a1" }, []);
		const second = worktree("second", { type: "InWorkspace", subject: "a1" }, []);
		const { worktrees } = layout([stackWith("shallow", ["a1"])], null, listing, folded, [
			first,
			second,
		]);
		expect(worktrees.on.get("a1")).toEqual([first, second]);
	});
});
