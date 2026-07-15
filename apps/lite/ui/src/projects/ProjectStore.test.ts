import {
	commitOperand,
	operandIdentityKey,
	uncommittedChangesOperand,
	type Operand,
} from "#ui/operands.ts";
import { ProjectStore } from "#ui/projects/ProjectStore.ts";
import { ProjectsStore } from "#ui/projects/ProjectsStore.ts";
import type { NavigationIndex } from "#ui/workspace/navigation-index.ts";
import type { RefInfo } from "@gitbutler/but-sdk";
import { autorun } from "mobx";
import { describe, expect, test } from "vitest";

const commit = commitOperand({ stackId: "stack", commitId: "commit" });
const otherCommit = commitOperand({ stackId: "stack", commitId: "other" });

const navigationIndex = (items: Array<Operand>): NavigationIndex<Operand> => ({
	items,
	indexByKey: new Map(items.map((item, index) => [operandIdentityKey(item), index])),
});

describe("ProjectStore", () => {
	test("resolves selection against the current navigation index", () => {
		const store = new ProjectStore();
		const index = navigationIndex([uncommittedChangesOperand, commit, otherCommit]);

		expect(store.selectedOutline(index)).toBe(uncommittedChangesOperand);

		store.selectOutline(otherCommit);
		expect(store.selectedOutline(index)).toBe(otherCommit);
		expect(store.isOutlineSelected(index, otherCommit)).toBe(true);

		const filteredIndex = navigationIndex([commit]);
		expect(store.selectedOutline(filteredIndex)).toBe(commit);
	});

	test("restores the selection when keyboard transfer mode is cancelled", () => {
		const store = new ProjectStore();
		store.selectOutline(commit);
		store.selectFiles("before.ts");
		store.enterKeyboardTransferMode(commit);
		store.selectFiles("after.ts");

		expect(store.outlineMode._tag).toBe("Transfer");

		store.cancelMode();

		expect(store.outlineMode._tag).toBe("Default");
		expect(store.outlineSelection).toBe(commit);
		expect(store.filesSelection).toBe("before.ts");
	});

	test("publishes computed checked-commit state reactively", () => {
		const store = new ProjectStore();
		const counts: Array<number> = [];
		const dispose = autorun(() => counts.push(store.checkedCommitCount));

		store.setCommitChecked("one", true);
		store.setCommitsChecked(["two", "three"], true);
		store.setCommitChecked("two", false);
		store.clearCheckedCommits();
		dispose();

		expect(counts).toEqual([0, 1, 3, 2, 0]);
		expect(store.hasCheckedCommits).toBe(false);
	});

	test("rewrites each checked commit once per backend response", () => {
		const store = new ProjectStore();
		store.setCommitChecked("old", true);
		store.setCommitTarget({ type: "commit", subject: "old" });

		store.updateRewrittenCommitReferences({ old: "new", new: "newest" }, {} as RefInfo);

		expect([...store.checkedCommitIds]).toEqual(["new"]);
		expect(store.commitTarget).toEqual({ type: "commit", subject: "new" });
	});

	test("keeps project state isolated while preserving store identity", () => {
		const projects = new ProjectsStore();
		const first = projects.getProject("first");
		first.openDialog({ _tag: "Settings" });
		first.toggleFiles();

		expect(projects.getProject("first")).toBe(first);
		expect(projects.getProject("first").dialog).toEqual({ _tag: "Settings" });
		expect(projects.getProject("first").filesVisible).toBe(true);
		expect(projects.getProject("second").dialog).toEqual({ _tag: "None" });
		expect(projects.getProject("second").filesVisible).toBe(false);
	});
});
