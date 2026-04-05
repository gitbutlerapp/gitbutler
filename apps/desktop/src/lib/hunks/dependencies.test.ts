import { filterDependenciesByAssignments } from "$lib/hunks/dependencies";
import { describe, expect, test } from "vitest";
import type { HunkDependencies } from "$lib/hunks/dependencies";
import type { HunkAssignment } from "$lib/hunks/hunk";

function makeDeps(
	entries: Array<{ path: string; newStart: number; newLines: number }>,
): HunkDependencies {
	return {
		diffs: entries.map(({ path, newStart, newLines }) => [
			path,
			{ oldStart: 1, oldLines: 1, newStart, newLines, diff: "" },
			[{ target: { type: "stack", subject: "stack-1" }, commitId: "abc" }],
		]),
		errors: [],
	};
}

function makeAssignment(
	path: string,
	stackId: string | null,
	newStart: number,
	newLines: number,
): HunkAssignment {
	return {
		id: null,
		path,
		pathBytes: [],
		stackId,
		hunkHeader: { oldStart: 1, oldLines: 1, newStart, newLines },
		lineNumsAdded: null,
		lineNumsRemoved: null,
	};
}

describe("filterDependenciesByAssignments", () => {
	describe("unassigned view (stackId = undefined)", () => {
		test("keeps dependency overlapping an unassigned hunk", () => {
			const deps = makeDeps([{ path: "a.ts", newStart: 10, newLines: 5 }]);
			const assignments = [makeAssignment("a.ts", null, 10, 5)];
			const result = filterDependenciesByAssignments(deps, assignments, undefined);
			expect(result.diffs).toHaveLength(1);
		});

		test("drops dependency overlapping a stack-assigned hunk", () => {
			const deps = makeDeps([{ path: "a.ts", newStart: 10, newLines: 5 }]);
			const assignments = [makeAssignment("a.ts", "stack-1", 10, 5)];
			const result = filterDependenciesByAssignments(deps, assignments, undefined);
			expect(result.diffs).toHaveLength(0);
		});

		test("keeps only the entry overlapping the unassigned hunk when mixed", () => {
			const deps = makeDeps([
				{ path: "a.ts", newStart: 10, newLines: 5 }, // overlaps unassigned hunk
				{ path: "a.ts", newStart: 50, newLines: 5 }, // overlaps stack-assigned hunk
			]);
			const assignments = [
				makeAssignment("a.ts", null, 10, 5),
				makeAssignment("a.ts", "stack-1", 50, 5),
			];
			const result = filterDependenciesByAssignments(deps, assignments, undefined);
			expect(result.diffs).toHaveLength(1);
			expect(result.diffs[0]![1].newStart).toBe(10);
		});
	});

	describe("stack lane view (stackId = 'stack-1')", () => {
		test("keeps dependency overlapping a hunk assigned to this stack", () => {
			const deps = makeDeps([{ path: "a.ts", newStart: 10, newLines: 5 }]);
			const assignments = [makeAssignment("a.ts", "stack-1", 10, 5)];
			const result = filterDependenciesByAssignments(deps, assignments, "stack-1");
			expect(result.diffs).toHaveLength(1);
		});

		test("drops dependency overlapping an unassigned hunk", () => {
			const deps = makeDeps([{ path: "a.ts", newStart: 10, newLines: 5 }]);
			const assignments = [makeAssignment("a.ts", null, 10, 5)];
			const result = filterDependenciesByAssignments(deps, assignments, "stack-1");
			expect(result.diffs).toHaveLength(0);
		});

		test("drops dependency overlapping a hunk from a different stack", () => {
			const deps = makeDeps([{ path: "a.ts", newStart: 10, newLines: 5 }]);
			const assignments = [makeAssignment("a.ts", "stack-2", 10, 5)];
			const result = filterDependenciesByAssignments(deps, assignments, "stack-1");
			expect(result.diffs).toHaveLength(0);
		});
	});

	describe("range overlap", () => {
		test("overlapping ranges match", () => {
			const deps = makeDeps([{ path: "a.ts", newStart: 10, newLines: 5 }]); // [10, 15)
			const assignments = [makeAssignment("a.ts", null, 12, 5)]; // [12, 17)
			const result = filterDependenciesByAssignments(deps, assignments, undefined);
			expect(result.diffs).toHaveLength(1);
		});

		test("adjacent but non-overlapping ranges do not match", () => {
			const deps = makeDeps([{ path: "a.ts", newStart: 10, newLines: 5 }]); // [10, 15)
			const assignments = [makeAssignment("a.ts", null, 15, 5)]; // [15, 20)
			const result = filterDependenciesByAssignments(deps, assignments, undefined);
			expect(result.diffs).toHaveLength(0);
		});

		test("0-line dependency hunk treated as single-line point", () => {
			const deps = makeDeps([{ path: "a.ts", newStart: 10, newLines: 0 }]); // treated as [10, 11)
			const assignments = [makeAssignment("a.ts", null, 10, 5)]; // [10, 15)
			const result = filterDependenciesByAssignments(deps, assignments, undefined);
			expect(result.diffs).toHaveLength(1);
		});

		test("0-line assignment hunk treated as single-line point", () => {
			const deps = makeDeps([{ path: "a.ts", newStart: 10, newLines: 5 }]); // [10, 15)
			const assignments = [makeAssignment("a.ts", null, 10, 0)]; // treated as [10, 11)
			const result = filterDependenciesByAssignments(deps, assignments, undefined);
			expect(result.diffs).toHaveLength(1);
		});

		test("no match when dependency is entirely before assignment", () => {
			const deps = makeDeps([{ path: "a.ts", newStart: 5, newLines: 3 }]); // [5, 8)
			const assignments = [makeAssignment("a.ts", null, 10, 5)]; // [10, 15)
			const result = filterDependenciesByAssignments(deps, assignments, undefined);
			expect(result.diffs).toHaveLength(0);
		});
	});

	describe("path matching", () => {
		test("only matches entries with the same path", () => {
			const deps = makeDeps([{ path: "a.ts", newStart: 10, newLines: 5 }]);
			const assignments = [makeAssignment("b.ts", null, 10, 5)];
			const result = filterDependenciesByAssignments(deps, assignments, undefined);
			expect(result.diffs).toHaveLength(0);
		});
	});

	describe("preserves errors", () => {
		test("errors field is passed through unchanged", () => {
			const deps: HunkDependencies = {
				diffs: [],
				errors: [{ errorMessage: "oops", stackId: "s", commitId: "c", path: "a.ts" }],
			};
			const result = filterDependenciesByAssignments(deps, [], undefined);
			expect(result.errors).toEqual(deps.errors);
		});
	});
});
