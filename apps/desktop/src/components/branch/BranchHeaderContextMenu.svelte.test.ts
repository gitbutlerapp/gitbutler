import { createBranchRelativeTo } from "$components/branch/BranchHeaderContextMenu.svelte";
import { describe, expect, test, vi } from "vitest";
import type { Segment } from "@gitbutler/but-sdk";

// The component's service imports reach Sentry, whose SvelteKit build needs the `$app` alias.
vi.mock("@sentry/sveltekit", () => ({ captureException: vi.fn() }));

// Only the fields the selector reads; `commits` is newest to oldest, as the backend projects it.
function segment(commitIds: string[]): Segment {
	return {
		refName: { displayName: "feature" },
		commits: commitIds.map((id) => ({ id })),
	} as unknown as Segment;
}

describe("createBranchRelativeTo", () => {
	test("managed below anchors at the bottom commit so the segment keeps its commits", () => {
		expect(createBranchRelativeTo(segment(["tip", "middle", "bottom"]), "below", true)).toEqual({
			type: "commit",
			subject: "bottom",
		});
	});

	test("managed above stays reference-relative", () => {
		expect(createBranchRelativeTo(segment(["tip", "bottom"]), "above", true)).toEqual({
			type: "reference",
			subject: "refs/heads/feature",
		});
	});

	test("managed below on an empty segment stays reference-relative", () => {
		expect(createBranchRelativeTo(segment([]), "below", true)).toEqual({
			type: "reference",
			subject: "refs/heads/feature",
		});
	});

	test("ad-hoc below stays reference-relative", () => {
		expect(createBranchRelativeTo(segment(["tip", "bottom"]), "below", false)).toEqual({
			type: "reference",
			subject: "refs/heads/feature",
		});
	});

	test("a segment without a name has no anchor", () => {
		const unnamed = { refName: null, commits: [] } as unknown as Segment;
		expect(createBranchRelativeTo(unnamed, "below", true)).toBeUndefined();
	});
});
