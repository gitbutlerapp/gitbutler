import { FileListController } from "$lib/selection/fileListController.svelte";
import { FileSelectionManager } from "$lib/selection/fileSelectionManager.svelte";
import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
import { createWorktreeSelection, type SelectionId } from "$lib/selection/key";
import { FOCUS_MANAGER } from "@gitbutler/ui/focus/focusManager";
import { get } from "svelte/store";
import { describe, expect, test, vi, beforeEach, type Mock } from "vitest";
import type { TreeChange } from "@gitbutler/but-sdk";

// Mock inject() to return test doubles keyed by token.
const injectMap = new Map<unknown, unknown>();
vi.mock("@gitbutler/core/context", () => ({
	InjectionToken: class {
		_key = Symbol();
	},
	inject(token: { _key: symbol }) {
		const value = injectMap.get(token);
		if (!value) throw new Error("No mock for token");
		return value;
	},
}));

function createManager(): FileSelectionManager {
	return new FileSelectionManager(null as any, null as any, null as any, null as any, null as any);
}

function makeChange(path: string): TreeChange {
	return { path } as TreeChange;
}

const CHANGES = ["a.ts", "b.ts", "c.ts", "d.ts", "e.ts"].map(makeChange);

function selectionId(): SelectionId {
	return createWorktreeSelection({ stackId: undefined });
}

function keyboardEvent(
	key: string,
	opts: { shiftKey?: boolean; ctrlKey?: boolean; metaKey?: boolean } = {},
): KeyboardEvent {
	return {
		key,
		shiftKey: opts.shiftKey ?? false,
		ctrlKey: opts.ctrlKey ?? false,
		metaKey: opts.metaKey ?? false,
		preventDefault: vi.fn(),
		stopPropagation: vi.fn(),
		currentTarget: document.createElement("div"),
	} as unknown as KeyboardEvent;
}

function mouseEvent(
	opts: { ctrlKey?: boolean; metaKey?: boolean; shiftKey?: boolean } = {},
): MouseEvent {
	return {
		ctrlKey: opts.ctrlKey ?? false,
		metaKey: opts.metaKey ?? false,
		shiftKey: opts.shiftKey ?? false,
	} as unknown as MouseEvent;
}

let manager: FileSelectionManager;
let focusManager: { focusByElement: Mock; activateOutline: Mock };

function createController(
	changes: TreeChange[] = CHANGES,
	sid: SelectionId = selectionId(),
	allowUnselect: boolean = true,
): FileListController {
	return new FileListController({
		changes: () => changes,
		selectionId: () => sid,
		allowUnselect: () => allowUnselect,
	});
}

function selectedPaths(ctrl: FileListController): string[] {
	return ctrl.selectedFileIds.map((f) => f.path);
}

beforeEach(() => {
	manager = createManager();
	focusManager = { focusByElement: vi.fn(), activateOutline: vi.fn() };
	injectMap.set(FILE_SELECTION_MANAGER, manager);
	injectMap.set(FOCUS_MANAGER, focusManager);
});

// ── Mouse selection ──────────────────────────────────────────────────────

describe("FileListController — mouse selection (select)", () => {
	test("plain click selects a single file", () => {
		$effect.root(() => {
			const ctrl = createController();
			ctrl.select(mouseEvent(), CHANGES[2]!, 2);

			expect(selectedPaths(ctrl)).toEqual(["c.ts"]);
		});
	});

	test("plain click on different file replaces selection", () => {
		$effect.root(() => {
			const ctrl = createController();
			ctrl.select(mouseEvent(), CHANGES[0]!, 0);
			ctrl.select(mouseEvent(), CHANGES[3]!, 3);

			expect(selectedPaths(ctrl)).toEqual(["d.ts"]);
		});
	});

	test("ctrl+click toggles file into selection", () => {
		$effect.root(() => {
			const ctrl = createController();
			ctrl.select(mouseEvent(), CHANGES[0]!, 0);
			ctrl.select(mouseEvent({ ctrlKey: true }), CHANGES[2]!, 2);

			expect(selectedPaths(ctrl).sort()).toEqual(["a.ts", "c.ts"]);
		});
	});

	test("ctrl+click on selected file removes it", () => {
		$effect.root(() => {
			const ctrl = createController();
			ctrl.select(mouseEvent(), CHANGES[0]!, 0);
			ctrl.select(mouseEvent({ ctrlKey: true }), CHANGES[2]!, 2);
			ctrl.select(mouseEvent({ ctrlKey: true }), CHANGES[0]!, 0);

			expect(selectedPaths(ctrl)).toEqual(["c.ts"]);
		});
	});

	test("meta+click works the same as ctrl+click", () => {
		$effect.root(() => {
			const ctrl = createController();
			ctrl.select(mouseEvent(), CHANGES[0]!, 0);
			ctrl.select(mouseEvent({ metaKey: true }), CHANGES[1]!, 1);

			expect(selectedPaths(ctrl).sort()).toEqual(["a.ts", "b.ts"]);
		});
	});

	test("shift+click selects a range", () => {
		$effect.root(() => {
			const ctrl = createController();
			ctrl.select(mouseEvent(), CHANGES[1]!, 1);
			ctrl.select(mouseEvent({ shiftKey: true }), CHANGES[3]!, 3);

			expect(selectedPaths(ctrl).sort()).toEqual(["b.ts", "c.ts", "d.ts"]);
		});
	});

	test("shift+click selects range backwards", () => {
		$effect.root(() => {
			const ctrl = createController();
			ctrl.select(mouseEvent(), CHANGES[3]!, 3);
			ctrl.select(mouseEvent({ shiftKey: true }), CHANGES[1]!, 1);

			expect(selectedPaths(ctrl).sort()).toEqual(["b.ts", "c.ts", "d.ts"]);
		});
	});

	test("plain click on sole selected file unselects when allowUnselect", () => {
		$effect.root(() => {
			const ctrl = createController(CHANGES, selectionId(), true);
			ctrl.select(mouseEvent(), CHANGES[0]!, 0);
			ctrl.select(mouseEvent(), CHANGES[0]!, 0);

			expect(selectedPaths(ctrl)).toEqual([]);
		});
	});

	test("plain click on sole selected file keeps it when allowUnselect is false", () => {
		$effect.root(() => {
			const ctrl = createController(CHANGES, selectionId(), false);
			ctrl.select(mouseEvent(), CHANGES[0]!, 0);
			ctrl.select(mouseEvent(), CHANGES[0]!, 0);

			expect(selectedPaths(ctrl)).toEqual(["a.ts"]);
		});
	});
});

// ── Keyboard navigation ─────────────────────────────────────────────────

describe("FileListController — keyboard navigation (handleNavigation)", () => {
	function setupWithSelection(index: number, changes: TreeChange[] = CHANGES): FileListController {
		const sid = selectionId();
		const ctrl = createController(changes, sid);
		manager.set(changes[index]!.path, sid, index);
		return ctrl;
	}

	test("ArrowDown moves selection to the next file", () => {
		$effect.root(() => {
			const ctrl = setupWithSelection(1);
			const idx = ctrl.handleNavigation(keyboardEvent("ArrowDown"));

			expect(idx).toBe(2);
			expect(selectedPaths(ctrl)).toEqual(["c.ts"]);
		});
	});

	test("ArrowUp moves selection to the previous file", () => {
		$effect.root(() => {
			const ctrl = setupWithSelection(2);
			const idx = ctrl.handleNavigation(keyboardEvent("ArrowUp"));

			expect(idx).toBe(1);
			expect(selectedPaths(ctrl)).toEqual(["b.ts"]);
		});
	});

	test("j/k vim keys work like ArrowDown/ArrowUp", () => {
		$effect.root(() => {
			const ctrl = setupWithSelection(1);
			ctrl.handleNavigation(keyboardEvent("j"));
			expect(selectedPaths(ctrl)).toEqual(["c.ts"]);

			ctrl.handleNavigation(keyboardEvent("k"));
			expect(selectedPaths(ctrl)).toEqual(["b.ts"]);
		});
	});

	test("ArrowDown at end of list does not change selection", () => {
		$effect.root(() => {
			const ctrl = setupWithSelection(4);
			ctrl.handleNavigation(keyboardEvent("ArrowDown"));

			expect(selectedPaths(ctrl)).toEqual(["e.ts"]);
		});
	});

	test("ArrowUp at start of list does not change selection", () => {
		$effect.root(() => {
			const ctrl = setupWithSelection(0);
			ctrl.handleNavigation(keyboardEvent("ArrowUp"));

			expect(selectedPaths(ctrl)).toEqual(["a.ts"]);
		});
	});

	test("Shift+ArrowDown extends selection downward", () => {
		$effect.root(() => {
			const ctrl = setupWithSelection(1);
			ctrl.handleNavigation(keyboardEvent("ArrowDown", { shiftKey: true }));

			expect(selectedPaths(ctrl).sort()).toEqual(["b.ts", "c.ts"]);
		});
	});

	test("Shift+ArrowUp extends selection upward", () => {
		$effect.root(() => {
			const ctrl = setupWithSelection(2);
			ctrl.handleNavigation(keyboardEvent("ArrowUp", { shiftKey: true }));

			expect(selectedPaths(ctrl).sort()).toEqual(["b.ts", "c.ts"]);
		});
	});

	test("Shift+Arrow extending then reversing shrinks selection", () => {
		$effect.root(() => {
			const ctrl = setupWithSelection(2);

			ctrl.handleNavigation(keyboardEvent("ArrowDown", { shiftKey: true }));
			ctrl.handleNavigation(keyboardEvent("ArrowDown", { shiftKey: true }));
			expect(selectedPaths(ctrl).sort()).toEqual(["c.ts", "d.ts", "e.ts"]);

			// Reverse — should shrink
			ctrl.handleNavigation(keyboardEvent("ArrowUp", { shiftKey: true }));
			expect(selectedPaths(ctrl).sort()).toEqual(["c.ts", "d.ts"]);
		});
	});

	test("ArrowDown with multi-selection collapses to bottom", () => {
		$effect.root(() => {
			const sid = selectionId();
			const ctrl = createController(CHANGES, sid);
			manager.add("a.ts", sid, 0);
			manager.add("b.ts", sid, 1);
			manager.add("c.ts", sid, 2);

			ctrl.handleNavigation(keyboardEvent("ArrowDown"));

			expect(selectedPaths(ctrl)).toEqual(["c.ts"]);
		});
	});

	test("ArrowUp with multi-selection collapses to top", () => {
		$effect.root(() => {
			const sid = selectionId();
			const ctrl = createController(CHANGES, sid);
			manager.add("b.ts", sid, 1);
			manager.add("c.ts", sid, 2);
			manager.add("d.ts", sid, 3);

			ctrl.handleNavigation(keyboardEvent("ArrowUp"));

			expect(selectedPaths(ctrl)).toEqual(["b.ts"]);
		});
	});

	test("Ctrl+A selects all files", () => {
		$effect.root(() => {
			const ctrl = setupWithSelection(0);
			ctrl.handleNavigation(keyboardEvent("a", { ctrlKey: true }));

			expect(selectedPaths(ctrl).sort()).toEqual(["a.ts", "b.ts", "c.ts", "d.ts", "e.ts"]);
		});
	});

	test("Meta+A selects all files", () => {
		$effect.root(() => {
			const ctrl = setupWithSelection(0);
			ctrl.handleNavigation(keyboardEvent("A", { metaKey: true }));

			expect(selectedPaths(ctrl).sort()).toEqual(["a.ts", "b.ts", "c.ts", "d.ts", "e.ts"]);
		});
	});

	test("Escape clears preview and returns undefined", () => {
		$effect.root(() => {
			const sid = selectionId();
			const ctrl = createController(CHANGES, sid);
			manager.set("b.ts", sid, 1);

			const idx = ctrl.handleNavigation(keyboardEvent("Escape"));

			expect(idx).toBeUndefined();
			expect(get(manager.getById(sid).lastAdded)).toBeUndefined();
		});
	});

	test("unrecognized key returns undefined and preserves selection", () => {
		$effect.root(() => {
			const ctrl = setupWithSelection(1);
			const idx = ctrl.handleNavigation(keyboardEvent("x"));

			expect(idx).toBeUndefined();
			expect(selectedPaths(ctrl)).toEqual(["b.ts"]);
		});
	});

	test("handleNavigation with empty selection returns undefined", () => {
		$effect.root(() => {
			const ctrl = createController();
			const idx = ctrl.handleNavigation(keyboardEvent("ArrowDown"));

			expect(idx).toBeUndefined();
		});
	});
});
