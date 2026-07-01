import { FileSelectionManager } from "$lib/selection/fileSelectionManager.svelte";
import {
	createWorktreeSelection,
	createCommitSelection,
	type SelectionId,
} from "$lib/selection/key";
import { get } from "svelte/store";
import { describe, expect, test } from "vitest";

function createManager(): FileSelectionManager {
	// The constructor deps are only used by treeChanges/hunkAssignments/changeByKey,
	// which we don't test here. Cast nulls to satisfy the signature.
	return new FileSelectionManager(null as any, null as any, null as any, null as any, null as any);
}

function worktreeId(stackId?: string): SelectionId {
	return createWorktreeSelection({ stackId });
}

function commitId(commitId: string, stackId?: string): SelectionId {
	return createCommitSelection({ commitId, stackId });
}

describe("FileSelectionManager", () => {
	describe("add and has", () => {
		test("added file is present", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("src/foo.ts", id, 0);

			expect(mgr.has("src/foo.ts", id)).toBe(true);
			expect(mgr.has("src/bar.ts", id)).toBe(false);
		});

		test("add sets lastAdded", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("src/foo.ts", id, 3);

			const lastAdded = get(mgr.getById(id).lastAdded);
			expect(lastAdded?.index).toBe(3);
		});

		test("multiple adds accumulate", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("a.ts", id, 0);
			mgr.add("b.ts", id, 1);
			mgr.add("c.ts", id, 2);

			expect(mgr.collectionSize(id)).toBe(3);
			expect(mgr.has("a.ts", id)).toBe(true);
			expect(mgr.has("b.ts", id)).toBe(true);
			expect(mgr.has("c.ts", id)).toBe(true);
		});
	});

	describe("set", () => {
		test("replaces existing selection with a single file", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("a.ts", id, 0);
			mgr.add("b.ts", id, 1);
			mgr.set("c.ts", id, 2);

			expect(mgr.collectionSize(id)).toBe(1);
			expect(mgr.has("a.ts", id)).toBe(false);
			expect(mgr.has("b.ts", id)).toBe(false);
			expect(mgr.has("c.ts", id)).toBe(true);
		});
	});

	describe("remove", () => {
		test("removes a file from selection", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("a.ts", id, 0);
			mgr.add("b.ts", id, 1);
			mgr.remove("a.ts", id);

			expect(mgr.has("a.ts", id)).toBe(false);
			expect(mgr.has("b.ts", id)).toBe(true);
			expect(mgr.collectionSize(id)).toBe(1);
		});

		test("clears lastAdded when the removed file was last added", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("a.ts", id, 0);
			mgr.remove("a.ts", id);

			expect(get(mgr.getById(id).lastAdded)).toBeUndefined();
		});

		test("preserves lastAdded when a different file is removed", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("a.ts", id, 0);
			mgr.add("b.ts", id, 1);
			mgr.remove("a.ts", id);

			const lastAdded = get(mgr.getById(id).lastAdded);
			expect(lastAdded?.index).toBe(1);
		});
	});

	describe("clear", () => {
		test("removes all files and resets lastAdded", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("a.ts", id, 0);
			mgr.add("b.ts", id, 1);
			mgr.clear(id);

			expect(mgr.collectionSize(id)).toBe(0);
			expect(mgr.hasItems(id)).toBe(false);
			expect(get(mgr.getById(id).lastAdded)).toBeUndefined();
		});
	});

	describe("clearPreview", () => {
		test("resets lastAdded without removing files", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("a.ts", id, 0);
			mgr.clearPreview(id);

			expect(mgr.has("a.ts", id)).toBe(true);
			expect(get(mgr.getById(id).lastAdded)).toBeUndefined();
		});
	});

	describe("addMany", () => {
		test("adds multiple files and sets lastAdded to the specified last file", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.addMany(["a.ts", "b.ts", "c.ts"], id, { path: "c.ts", index: 2 });

			expect(mgr.collectionSize(id)).toBe(3);
			expect(mgr.has("a.ts", id)).toBe(true);
			expect(mgr.has("b.ts", id)).toBe(true);
			expect(mgr.has("c.ts", id)).toBe(true);

			const lastAdded = get(mgr.getById(id).lastAdded);
			expect(lastAdded?.index).toBe(2);
		});
	});

	describe("values", () => {
		test("returns parsed SelectedFile objects", () => {
			const mgr = createManager();
			const id = worktreeId("stack-1");

			mgr.add("a.ts", id, 0);
			mgr.add("b.ts", id, 1);

			const vals = mgr.values(id);
			expect(vals).toHaveLength(2);
			expect(vals.map((v) => v.path).sort()).toEqual(["a.ts", "b.ts"]);
			expect(vals[0]!.type).toBe("worktree");
		});
	});

	describe("collectionSize and hasItems", () => {
		test("empty selection", () => {
			const mgr = createManager();
			const id = commitId("abc123");

			expect(mgr.collectionSize(id)).toBe(0);
			expect(mgr.hasItems(id)).toBe(false);
		});

		test("non-empty selection", () => {
			const mgr = createManager();
			const id = commitId("abc123");

			mgr.add("x.ts", id, 0);

			expect(mgr.collectionSize(id)).toBe(1);
			expect(mgr.hasItems(id)).toBe(true);
		});
	});

	describe("selection isolation", () => {
		test("different selection ids are independent", () => {
			const mgr = createManager();
			const worktree = worktreeId();
			const commit = commitId("abc123");

			mgr.add("a.ts", worktree, 0);
			mgr.add("b.ts", commit, 0);

			expect(mgr.has("a.ts", worktree)).toBe(true);
			expect(mgr.has("b.ts", worktree)).toBe(false);
			expect(mgr.has("b.ts", commit)).toBe(true);
			expect(mgr.has("a.ts", commit)).toBe(false);
		});

		test("clearing one selection does not affect another", () => {
			const mgr = createManager();
			const worktree = worktreeId();
			const commit = commitId("abc123");

			mgr.add("a.ts", worktree, 0);
			mgr.add("b.ts", commit, 0);
			mgr.clear(worktree);

			expect(mgr.collectionSize(worktree)).toBe(0);
			expect(mgr.has("b.ts", commit)).toBe(true);
		});
	});

	describe("retain", () => {
		test("removes worktree selections not in the provided paths", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("a.ts", id, 0);
			mgr.add("b.ts", id, 1);
			mgr.add("c.ts", id, 2);

			mgr.retain(["a.ts", "c.ts"]);

			expect(mgr.has("a.ts", id)).toBe(true);
			expect(mgr.has("b.ts", id)).toBe(false);
			expect(mgr.has("c.ts", id)).toBe(true);
		});

		test("clears all selections when called with undefined", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("a.ts", id, 0);
			mgr.retain(undefined);

			// After clearing, a fresh getById will create a new empty selection
			expect(mgr.collectionSize(id)).toBe(0);
		});

		test("does nothing when all paths are retained", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("a.ts", id, 0);
			mgr.add("b.ts", id, 1);

			mgr.retain(["a.ts", "b.ts", "c.ts"]);

			expect(mgr.collectionSize(id)).toBe(2);
		});
	});

	describe("removeMany", () => {
		test("removes multiple files at once", () => {
			const mgr = createManager();
			const id = worktreeId();

			mgr.add("a.ts", id, 0);
			mgr.add("b.ts", id, 1);
			mgr.add("c.ts", id, 2);

			const toRemove = mgr.values(id).filter((f) => f.path !== "b.ts");
			mgr.removeMany(toRemove);

			expect(mgr.collectionSize(id)).toBe(1);
			expect(mgr.has("b.ts", id)).toBe(true);
		});
	});

	describe("getById", () => {
		test("lazily creates selections for unknown ids", () => {
			const mgr = createManager();
			const id = commitId("new-commit");

			const selection = mgr.getById(id);

			expect(selection.entries.size).toBe(0);
			expect(get(selection.lastAdded)).toBeUndefined();
		});

		test("returns the same selection on repeated calls", () => {
			const mgr = createManager();
			const id = commitId("abc");

			const first = mgr.getById(id);
			const second = mgr.getById(id);

			expect(first).toBe(second);
		});
	});
});
