import type { Stack, TargetCommit, TargetCommitPage } from "@gitbutler/but-sdk";
import { describe, expect, it } from "vitest";
import type { Address } from "#ui/addresses.ts";
import { foldAt, layout, FIRST, MORE, sectionAddresses, targetCommitAddress } from "./layout.ts";

type Folds = Parameters<typeof layout>[3];

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

const folded: Folds = {
	incomingExpanded: false,
	baseExpanded: false,
	moreRuns: {},
	moreOlder: 0,
};
const expanded: Folds = { ...folded, incomingExpanded: true, baseExpanded: true };

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

// The listing with two commits below the deepest fork point.
const listingWithTail: TargetCommitPage = {
	commits: [...listing.commits, commit("t1", true), commit("t2", true)],
	hasMore: false,
};

describe("layout", () => {
	it("shows the listing's tail and the pages fetched below the base once it is open", () => {
		const stacks = [stack("deep"), stack("shallow")];
		const pages = [commit("o1", true), commit("o2", true)];
		const shown = layout(stacks, null, listingWithTail, expanded, pages);
		// The base heads the section itself; the rows under it start below it.
		expect(shown.base?.commit.id).toBe("shallow");
		expect(shown.belowBase.map((item) => item.kind)).toEqual(["run", "fork"]);
		expect(shown.older.map((c) => c.commit.id)).toEqual(["t1", "t2", "o1", "o2"]);
		// The base's fold hides them with the rows above.
		expect(layout(stacks, null, listingWithTail, folded, pages).older).toEqual([]);
	});

	it("shows the older history a first helping at a time, and more with each ask", () => {
		const stacks = [stack("deep"), stack("shallow")];
		const pages = Array.from({ length: 40 }, (_, i) => commit(`o${i}`, true));
		const all = layout(stacks, null, listingWithTail, { ...expanded, moreOlder: 99 }, pages).older;
		expect(all.length).toBeGreaterThan(FIRST + MORE);
		const first = layout(stacks, null, listingWithTail, expanded, pages);
		expect(first.older).toEqual(all.slice(0, FIRST));
		expect(first.olderHidden).toBe(all.length - FIRST);
		const more = layout(stacks, null, listingWithTail, { ...expanded, moreOlder: 1 }, pages);
		expect(more.older).toHaveLength(FIRST + MORE);
		expect(more.olderHidden).toBe(all.length - FIRST - MORE);
	});

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

	it("folds the incoming leg under the upstream header and the base rows under the base's", () => {
		const plan = layout([stack("deep")], null, listing, folded);
		expect(plan.belowBase).toEqual([]);
		expect(plan.incoming).toEqual([]);
		// The base header names the fork point nearest the tip either way.
		expect(plan.base?.commit.id).toBe("deep");
		expect(layout([stack("deep"), stack("shallow")], null, listing, folded).base?.commit.id).toBe(
			"shallow",
		);
		expect(
			layout([stack("deep")], null, listing, { ...folded, baseExpanded: true }).belowBase.map(
				(item) => item.kind,
			),
		).toEqual(["run"]);
	});

	it("puts the incoming commits on their own leg, ahead of the base", () => {
		const plan = layout([stack("deep"), stack("shallow")], null, listing, expanded);
		expect(plan.incoming.map((run) => run.shown.map((c) => c.commit.id))).toEqual([["i1", "i2"]]);
		expect(plan.belowBase.map((item) => item.kind)).toEqual(["run", "fork"]);
	});

	it("lists the section's rows as values and knows which fold each sits in", () => {
		const plan = layout([stack("deep"), stack("shallow")], null, listing, expanded);
		const ids = (addresses: Array<Address>) =>
			addresses.map((address) => (address._tag === "Commit" ? address.commitId : address._tag));
		// Incoming commits, then what lies below the base; the workspace's own
		// run folds entirely, and the headers, the base's included, are not values.
		expect(ids(sectionAddresses(plan))).toEqual(["i1", "i2", "deep"]);
		const at = (id: string) => foldAt(plan, targetCommitAddress(commit(id, true)));
		expect(at("i1")).toBe("incoming");
		expect(at("deep")).toBe("base");
		expect(at("shallow")).toBeNull();
		expect(at("h1")).toBeNull();
	});

	it("shows an incoming run's newest first, more with each ask, unless a fold would hide one row", () => {
		const ids = Array.from({ length: 25 }, (_, i) => `c${i}`);
		const many = incomingAbove(ids);
		const run = layout([stack("base")], null, many, expanded).incoming[0];
		expect(run?.kind === "run" && run.shown.length).toBe(FIRST);
		expect(run?.kind === "run" && run.hidden).toBe(25 - FIRST);
		expect(run?.kind === "run" && run.expanded).toBe(false);
		const asked = { ...expanded, moreRuns: { c0: 1 } };
		const more = layout([stack("base")], null, many, asked).incoming[0];
		expect(more?.kind === "run" && more.shown.length).toBe(25);
		expect(more?.kind === "run" && more.hidden).toBe(0);
		expect(more?.kind === "run" && more.expanded).toBe(true);

		const eleven = incomingAbove(ids.slice(0, FIRST + 1));
		const whole = layout([stack("base")], null, eleven, expanded).incoming[0];
		expect(whole?.kind === "run" && whole.shown.length).toBe(FIRST + 1);
		expect(whole?.kind === "run" && whole.hidden).toBe(0);
	});

	it("folds a run the workspace already has entirely, and reveals it on asking", () => {
		const items = layout([stack("deep"), stack("shallow")], null, listing, expanded).belowBase;
		expect(items.map((item) => item.kind)).toEqual(["run", "fork"]);
		const had = items[0];
		expect(had?.kind === "run" && had.shown.length).toBe(0);
		expect(had?.kind === "run" && had.hidden).toBe(3);
		const asked = { ...expanded, moreRuns: { h1: 1 } };
		const shown = layout([stack("deep"), stack("shallow")], null, listing, asked).belowBase[0];
		expect(shown?.kind === "run" && shown.shown.map((c) => c.commit.id)).toEqual([
			"h1",
			"h2",
			"h3",
		]);
	});

	it("knows when the target's tip is the base itself", () => {
		const current: TargetCommitPage = { commits: [commit("shallow", true)], hasMore: false };
		expect(layout([stack("shallow")], null, current, folded).refOnBase).toBe(true);
	});
});
