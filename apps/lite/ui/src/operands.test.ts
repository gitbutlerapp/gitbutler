import {
	commitFileParent,
	commitOperand,
	fileOperand,
	filesUnder,
	uncommittedChangesFileParent,
} from "./operands.ts";
import { describe, expect, it } from "vitest";

describe("filesUnder", () => {
	const commit = commitFileParent({ commitId: "a1", changeId: "change-a1" });
	const otherCommit = commitFileParent({ commitId: "b2", changeId: "change-b2" });

	it("keeps files sharing the given parent", () => {
		const files = [
			fileOperand({ parent: commit, path: "one.ts" }),
			fileOperand({ parent: commit, path: "two.ts" }),
		];

		expect(filesUnder(files, commit)).toEqual(files);
	});

	it("drops a set holding a file of another commit", () => {
		expect(
			filesUnder(
				[
					fileOperand({ parent: commit, path: "one.ts" }),
					fileOperand({ parent: otherCommit, path: "two.ts" }),
				],
				commit,
			),
		).toEqual([]);
	});

	it("drops a set holding an uncommitted file", () => {
		expect(
			filesUnder(
				[
					fileOperand({ parent: commit, path: "one.ts" }),
					fileOperand({ parent: uncommittedChangesFileParent, path: "two.ts" }),
				],
				commit,
			),
		).toEqual([]);
	});

	it("drops a set holding a commit", () => {
		expect(
			filesUnder(
				[
					fileOperand({ parent: uncommittedChangesFileParent, path: "one.ts" }),
					commitOperand({ commitId: "a1", changeId: "change-a1" }),
				],
				uncommittedChangesFileParent,
			),
		).toEqual([]);
	});

	it("has nothing to keep when nothing is checked", () => {
		expect(filesUnder([], uncommittedChangesFileParent)).toEqual([]);
	});
});
