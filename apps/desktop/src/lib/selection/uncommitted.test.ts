import {
	uncommittedActions,
	uncommittedSelectors,
	uncommittedSlice,
} from "$lib/selection/uncommitted";
import { describe, expect, test } from "vitest";
import type { HunkAssignment } from "@gitbutler/but-sdk";

function assignment(path: string): HunkAssignment {
	return {
		id: path,
		hunkHeader: null,
		path,
		pathBytes: Array.from(new TextEncoder().encode(path)),
		stackId: null,
		branchRefBytes: null,
		lineNumsAdded: null,
		lineNumsRemoved: null,
	};
}

describe("uncommitted selection", () => {
	test("checks every file below a folder path", () => {
		let state = uncommittedSlice.reducer(
			undefined,
			uncommittedActions.update({
				assignments: [
					assignment("src/a.ts"),
					assignment("src/nested/b.ts"),
					assignment("other/c.ts"),
				],
				changes: [],
			}),
		);

		state = uncommittedSlice.reducer(
			state,
			uncommittedActions.checkDir({ stackId: null, path: "src" }),
		);

		expect(
			uncommittedSelectors.hunkSelection.selectAll(state.hunkSelection).map((item) => item.path),
		).toEqual(["src/a.ts", "src/nested/b.ts"]);
		expect(
			uncommittedSelectors.hunkSelection.folderCheckStatus(state, {
				stackId: null,
				path: "src",
			}),
		).toBe("checked");
	});

	test("unchecks only the selected file paths", () => {
		let state = uncommittedSlice.reducer(
			undefined,
			uncommittedActions.update({
				assignments: [assignment("src/a.ts"), assignment("src/b.ts"), assignment("other/c.ts")],
				changes: [],
			}),
		);

		state = uncommittedSlice.reducer(
			state,
			uncommittedActions.checkFiles({ stackId: null, paths: ["src/a.ts", "other/c.ts"] }),
		);
		state = uncommittedSlice.reducer(
			state,
			uncommittedActions.uncheckFiles({ stackId: null, paths: ["src/a.ts"] }),
		);

		expect(
			uncommittedSelectors.hunkSelection.selectAll(state.hunkSelection).map((item) => item.path),
		).toEqual(["other/c.ts"]);
	});
});
