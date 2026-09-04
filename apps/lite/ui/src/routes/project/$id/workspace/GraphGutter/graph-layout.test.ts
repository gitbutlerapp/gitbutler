import type { Stack, TargetCommit, TargetCommitPage } from "@gitbutler/but-sdk";
import { describe, expect, it } from "vitest";
import { planGutter, railPaths } from "./graph-layout.ts";

type Folds = Parameters<typeof planGutter>[3];
type Anchors = Parameters<typeof railPaths>[1];

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
	expandedRuns: new Set(),
	olderShown: false,
};
const expanded: Folds = { ...folded, incomingExpanded: true, baseExpanded: true };

// Newest first: two incoming, a shallow base, three had, a deep base.
const trunk: TargetCommitPage = {
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
const trunkWithTail: TargetCommitPage = {
	commits: [...trunk.commits, commit("t1", true), commit("t2", true)],
	hasMore: false,
};

describe("planGutter", () => {
	it("keeps the listing's tail hidden until older history is asked for, then heads it with it", () => {
		const stacks = [stack("deep"), stack("shallow")];
		const hidden = planGutter(stacks, null, trunkWithTail, expanded);
		expect(hidden.older).toEqual([]);
		expect(hidden.olderHidden).toBe(2);
		expect(hidden.trunk.map((item) => item.kind)).toEqual(["fork", "run", "fork"]);

		const pages = [commit("o1", true), commit("o2", true)];
		const shown = planGutter(stacks, null, trunkWithTail, { ...expanded, olderShown: true }, pages);
		expect(shown.older.map((c) => c.commit.id)).toEqual(["t1", "t2", "o1", "o2"]);
		expect(shown.olderHidden).toBe(0);
		// The base's fold hides it with the base rows; nothing is forgotten.
		expect(
			planGutter(stacks, null, trunkWithTail, { ...folded, olderShown: true }, pages).older,
		).toEqual([]);
		expect(planGutter(stacks, null, trunkWithTail, folded).olderHidden).toBe(2);
	});

	it("orders other bases' cards first by depth, then the spine's, each on or beside its line", () => {
		const plan = planGutter([stack("deep"), stack("shallow"), stack("deep")], null, trunk, folded);
		expect(plan.spine).toBe(0);
		expect(plan.order).toEqual([1, 0, 2]);
		expect(plan.columnOf).toEqual([1, 0, 0]);
		// The shallow base's line outermost, the parent line next to the rail;
		// every card sits on the one rail, right of both.
		expect(plan.lines).toEqual([1, 0]);
		expect(plan.railX).toBe(36);
	});

	it("gives only the lines some card bends onto their room, half a gap with none", () => {
		expect(planGutter([stack("deep")], null, trunk, folded)).toMatchObject({
			lines: [],
			railX: 18,
		});
		expect(planGutter([stack("deep"), stack("deep")], null, trunk, folded)).toMatchObject({
			lines: [0],
			railX: 24,
		});
		// One stack per base: the deep card bends onto its line, the shallow
		// one, the spine's last, runs straight on.
		expect(planGutter([stack("deep"), stack("shallow")], null, trunk, folded)).toMatchObject({
			lines: [0],
			railX: 24,
		});
	});

	it("makes the base most stacks sit on the spine, newest on a tie", () => {
		expect(
			planGutter([stack("deep"), stack("shallow"), stack("deep")], null, trunk, folded).spine,
		).toBe(0);
		expect(planGutter([stack("deep"), stack("shallow")], null, trunk, folded).spine).toBe(1);
	});

	it("folds the incoming leg under the upstream header and the base rows under the base's", () => {
		const plan = planGutter([stack("deep")], null, trunk, folded);
		expect(plan.trunk).toEqual([]);
		expect(plan.incoming).toEqual([]);
		// The base header names the fork point nearest the tip either way.
		expect(plan.base?.commit.id).toBe("deep");
		expect(planGutter([stack("deep"), stack("shallow")], null, trunk, folded).base?.commit.id).toBe(
			"shallow",
		);
		expect(
			planGutter([stack("deep")], null, trunk, { ...folded, baseExpanded: true }).trunk.at(-1),
		).toMatchObject({ kind: "fork" });
	});

	it("puts the incoming commits on their own leg, ahead of the base", () => {
		const plan = planGutter([stack("deep"), stack("shallow")], null, trunk, expanded);
		expect(plan.incoming.map((run) => run.preview.map((c) => c.commit.id))).toEqual([["i1", "i2"]]);
		expect(plan.trunk.map((item) => item.kind)).toEqual(["fork", "run", "fork"]);
	});

	it("previews an incoming run and folds the rest, unless that would hide one row", () => {
		const many = incomingAbove(["a", "b", "c", "d", "e"]);
		const run = planGutter([stack("base")], null, many, expanded).incoming[0];
		expect(run?.kind === "run" && run.preview.map((c) => c.commit.id)).toEqual(["a", "b", "c"]);
		expect(run?.kind === "run" && run.rest.length).toBe(2);

		const four = incomingAbove(["a", "b", "c", "d"]);
		const whole = planGutter([stack("base")], null, four, expanded).incoming[0];
		expect(whole?.kind === "run" && whole.preview.length).toBe(4);
		expect(whole?.kind === "run" && whole.rest.length).toBe(0);
	});

	it("folds a run the workspace already has entirely", () => {
		const items = planGutter([stack("deep"), stack("shallow")], null, trunk, expanded).trunk;
		expect(items.map((item) => item.kind)).toEqual(["fork", "run", "fork"]);
		const had = items[1];
		expect(had?.kind === "run" && had.preview.length).toBe(0);
		expect(had?.kind === "run" && had.rest.map((c) => c.commit.id)).toEqual(["h1", "h2", "h3"]);
	});
});

describe("railPaths", () => {
	// Folded: the base header a card gap under the upstream header. Open: the
	// incoming card in that gap first, then the base header a gap under it,
	// and the base rows straight under the header.
	const anchors = (open: boolean): Anchors => ({
		cards: [
			{ topY: 0, exitY: 80, bottomY: 100 },
			{ topY: 120, exitY: 180, bottomY: 200 },
			{ topY: 220, exitY: 280, bottomY: 300 },
		],
		cardsEnd: 300,
		header: { y: 360, topY: 346, bottomY: 374 },
		leg: open ? { card: { topY: 394, bottomY: 418 }, rows: { topY: 400, bottomY: 412 } } : null,
		base: open ? { y: 452, topY: 438 } : { y: 408, topY: 394 },
		baseRowsTopY: open ? 466 : null,
	});
	it("draws a line faint behind the cards it passes and solid between them", () => {
		const stacks = [stack("deep"), stack("shallow"), stack("shallow")];
		const rails = railPaths(planGutter(stacks, null, trunk, expanded), anchors(true));
		// The deep card comes first; its column runs behind the two shallow
		// cards below, as does the parent line behind the last one. Open, the
		// trunk rail runs behind the incoming leg's card the same way.
		expect(rails.filter((r) => r.through).map((r) => r.d)).toEqual([
			"M 12 120 L 12 200",
			"M 12 220 L 12 300",
			"M 24 220 L 24 300",
			"M 36 394 L 36 418",
		]);
		expect(rails.filter((r) => !r.through).map((r) => r.d)).toContain("M 12 200 L 12 220");
	});

	it("bends every card's rail off its floor onto its line, the last one running straight on", () => {
		const stacks = [stack("deep"), stack("shallow"), stack("shallow")];
		const paths = railPaths(planGutter(stacks, null, trunk, folded), anchors(false)).map(
			(r) => r.d,
		);
		// The deep card bends left onto its column at the edge, the first
		// shallow card onto the parent line beside the rail; the last card's
		// rail runs straight on to the cards' end.
		expect(
			paths.some((d) => d.startsWith("M 36 80 L 36 100 L 36 106 C ") && d.endsWith("L 12 120")),
		).toBe(true);
		expect(
			paths.some((d) => d.startsWith("M 36 180 L 36 200 L 36 206 C ") && d.endsWith("L 24 220")),
		).toBe(true);
		expect(paths).toContain("M 36 280 L 36 300");
		// Above the header both lines turn right onto one row into the trunk
		// rail. With commits incoming the target has moved on, so the trunk
		// runs straight past the header into the base header's chevron, and
		// the leg bends into it from the chevron through the gap between.
		expect(paths).toContain("M 12 300 L 12 306 C 12 308.21 13.79 310 16 310");
		expect(paths).toContain("M 24 300 L 24 306 C 24 308.21 25.79 310 28 310");
		expect(paths).toContain("M 16 310 L 36 310");
		expect(paths).toContain("M 36 300 L 36 398");
		expect(paths.some((d) => d.startsWith("M 48 370 L 48 374 ") && d.endsWith(" 36 394"))).toBe(
			true,
		);
		// Folded, nothing runs out of the base header.
		expect(paths.some((d) => d.includes(" 418"))).toBe(false);
	});

	it("bends the leg's join through the gap under its card like a card's exit", () => {
		const stacks = [stack("deep"), stack("shallow"), stack("shallow")];
		const paths = railPaths(planGutter(stacks, null, trunk, expanded), anchors(true)).map(
			(r) => r.d,
		);
		// Straight to the card's edge at 418, then the S-bend centred in the
		// 20px gap to the base header at 438: its turn starts at 424.
		expect(paths.some((d) => d.startsWith("M 48 412 L 48 418 L 48 424 C "))).toBe(true);
		expect(paths).toContain("M 48 370 L 48 400");
		// Out of the base header's chevron into the base rows.
		expect(paths).toContain("M 36 462 L 36 466");
	});

	it("draws nothing below the header without a base to run into", () => {
		const stacks = [stack("deep"), stack("shallow"), stack("shallow")];
		const paths = railPaths(planGutter(stacks, null, trunk, folded), {
			...anchors(false),
			base: null,
		}).map((r) => r.d);
		expect(paths.some((d) => d.startsWith("M 36 300"))).toBe(false);
		expect(paths.some((d) => d.startsWith("M 48 370"))).toBe(false);
	});

	it("runs the trunk through the ref row when the target is an ancestor", () => {
		const current: TargetCommitPage = { commits: [commit("shallow", true)], hasMore: false };
		const paths = railPaths(planGutter([stack("shallow")], null, current, folded), {
			...anchors(false),
			cards: [{ topY: 0, exitY: 80, bottomY: 100 }],
			cardsEnd: 100,
			header: { y: 160, topY: 146, bottomY: 174 },
			base: { y: 208, topY: 194 },
		}).map((r) => r.d);
		// To the row's top edge, whose glyph carries the rail through it, and
		// on from its bottom edge into the base header's chevron.
		expect(paths).toContain("M 18 100 L 18 146");
		expect(paths).toContain("M 18 174 L 18 198");
	});
});
