import { AbsorptionTarget, HunkHeader, WorktreeChanges } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { Item } from "./Item.ts";

const hunkHeadersEqual = (a: HunkHeader, b: HunkHeader): boolean =>
	a.oldStart === b.oldStart &&
	a.oldLines === b.oldLines &&
	a.newStart === b.newStart &&
	a.newLines === b.newLines;

export const resolveAbsorptionTarget = ({
	item,
	worktreeChanges,
}: {
	item: Item;
	worktreeChanges: WorktreeChanges;
}): AbsorptionTarget | null =>
	Match.value(item).pipe(
		Match.withReturnType<AbsorptionTarget | null>(),
		Match.tag("ChangesSection", () => ({ type: "all" })),
		Match.when({ _tag: "File", parent: { _tag: "Changes" } }, ({ path }) => {
			const change = worktreeChanges.changes.find((candidate) => candidate.path === path);
			if (!change) return null;

			return {
				type: "treeChanges",
				subject: {
					changes: [change],
					assignedStackId: null,
				},
			};
		}),
		Match.when({ _tag: "Hunk", parent: { _tag: "Changes" } }, ({ path, hunkHeader }) => {
			const assignment = worktreeChanges.assignments.find(
				(candidate) =>
					candidate.path === path &&
					candidate.hunkHeader !== null &&
					hunkHeadersEqual(candidate.hunkHeader, hunkHeader),
			);
			if (!assignment) return null;

			return {
				type: "hunkAssignments",
				subject: {
					assignments: [assignment],
				},
			};
		}),
		Match.orElse(() => null),
	);
