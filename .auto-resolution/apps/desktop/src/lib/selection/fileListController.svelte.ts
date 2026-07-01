/**
 * Reactive controller that owns file list selection, keyboard navigation,
 * and focus management.
 *
 * Instantiate in a component's `<script>` block so that `inject()` and
 * `$effect()` bind to the component lifecycle.
 *
 * ```svelte
 * <script lang="ts">
 *   const controller = new FileListController({ ... });
 * </script>
 * ```
 */
import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
import { type SelectionId } from "$lib/selection/key";
import { inject } from "@gitbutler/core/context";
import { FOCUS_MANAGER } from "@gitbutler/ui/focus/focusManager";
import { getContext, setContext, untrack } from "svelte";
import { get } from "svelte/store";
import type { FileSelectionManager } from "$lib/selection/fileSelectionManager.svelte";
import type { SelectedFile } from "$lib/selection/key";
import type { TreeChange } from "@gitbutler/but-sdk";

const FILE_LIST_CTX = Symbol("FileListController");

/** Set the controller into Svelte component context. Called by FileListProvider. */
export function setFileListContext(controller: FileListController): void {
	setContext(FILE_LIST_CTX, controller);
}

/** Read the controller from Svelte component context. Called by compound children. */
export function getFileListContext(): FileListController {
	const ctx = getContext<FileListController>(FILE_LIST_CTX);
	if (!ctx) {
		throw new Error("FileListController not found — wrap your component in <FileListProvider>");
	}
	return ctx;
}

/**
 * Extra keyboard handler that callers can inject to extend file list
 * keyboard behavior (e.g. AI shortcuts in the worktree context).
 *
 * Return `true` to indicate the event was handled.
 */
export type FileListKeyHandler = (
	change: TreeChange,
	idx: number,
	e: KeyboardEvent,
) => boolean | void;

export class FileListController {
	private idSelection: FileSelectionManager;
	private focusManager;
	private getChanges: () => TreeChange[];
	private getSelectionId: () => SelectionId;
	private getAllowUnselect: () => boolean;
	// Tracks whether the current selection change was explicitly triggered by a
	// keyboard action handled by this controller (arrow nav or Enter/Space/l on
	// a file item). Only when true should the focus ring be activated. This
	// correctly excludes mouse clicks, folder clicks, and programmatic changes
	// that should not show the ring.
	private _selectionFromKeyboard = false;

	active = $state(false);

	/** True while handleNavigation/handleActivation is on the call stack. */
	get isKeyboardSelecting(): boolean {
		return this._selectionFromKeyboard;
	}
	readonly selectedPaths = $derived(new Set(this.selectedFileIds.map((f) => f.path)));
	readonly hasSelectionInList = $derived(
		this.changes.some((change) => this.selectedPaths.has(change.path)),
	);

	constructor(params: {
		changes: () => TreeChange[];
		selectionId: () => SelectionId;
		allowUnselect?: () => boolean;
	}) {
		this.idSelection = inject(FILE_SELECTION_MANAGER);
		this.focusManager = inject(FOCUS_MANAGER);
		this.getChanges = params.changes;
		this.getSelectionId = params.selectionId;
		this.getAllowUnselect = params.allowUnselect ?? (() => true);

		$effect(() => {
			const store = this.idSelection.getById(this.getSelectionId()).lastAdded;
			return store.subscribe((value) => {
				if (value?.index !== undefined) {
					untrack(() => {
						// Skip selections that came from a mouse click: FM's own click
						// handler already moved the cursor and the ring should stay hidden.
						// For keyboard navigation (arrows, Enter/Space) we look up the
						// selected file's DOM element by id (value.key == the element's DOM
						// id set in FileListItem) and focus it directly. This avoids the
						// flat-index → FM-children-index mismatch that caused infinite loops
						// in tree mode when focusNthSibling accidentally landed on a folder.
						if (this.active && this._selectionFromKeyboard) {
							const el = document.getElementById(value.key);
							if (el) {
								this.focusManager.focusByElement(el);
								this.focusManager.activateOutline();
							}
						}
					});
				}
			});
		});
	}

	get selection(): FileSelectionManager {
		return this.idSelection;
	}

	get selectionId(): SelectionId {
		return this.getSelectionId();
	}

	get changes(): TreeChange[] {
		return this.getChanges();
	}

	get selectedFileIds(): SelectedFile[] {
		return this.idSelection.values(this.selectionId);
	}

	isSelected(path: string): boolean {
		return this.idSelection.has(path, this.selectionId);
	}

	select(e: MouseEvent | KeyboardEvent, change: TreeChange, index: number): void {
		const isAlreadySelected = this.idSelection.has(change.path, this.selectionId);
		const isTheOnlyOneSelected =
			this.idSelection.collectionSize(this.selectionId) === 1 && isAlreadySelected;
		const lastAdded = get(this.idSelection.getById(this.selectionId).lastAdded);

		if (e.ctrlKey || e.metaKey) {
			if (isAlreadySelected) {
				this.idSelection.remove(change.path, this.selectionId);
				const remainingSelection = this.idSelection.values(this.selectionId);
				const previous = remainingSelection.at(-1);
				if (previous) {
					const previousIndex = this.changes.findIndex((file) => file.path === previous.path);
					if (previousIndex !== -1) {
						this.idSelection.add(previous.path, this.selectionId, previousIndex);
					}
				}
			} else {
				this.idSelection.add(change.path, this.selectionId, index);
			}
		} else if (e.shiftKey && lastAdded !== undefined) {
			const start = Math.min(lastAdded.index, index);
			const end = Math.max(lastAdded.index, index);

			const filePaths = this.changes.slice(start, end + 1).map((f) => f.path);
			this.idSelection.addMany(filePaths, this.selectionId, {
				path: change.path,
				index,
			});
		} else {
			if (isTheOnlyOneSelected) {
				if (this.getAllowUnselect()) {
					this.idSelection.clear(this.selectionId);
				}
			} else {
				this.idSelection.set(change.path, this.selectionId, index);
			}
		}
	}

	/** Returns true if the key was an activation key (Enter/Space/l) and select was called. */
	handleActivation(change: TreeChange, idx: number, e: KeyboardEvent): boolean {
		if (e.key === "Enter" || e.key === " " || e.key === "l") {
			e.stopPropagation();
			this._selectionFromKeyboard = true;
			this.select(e, change, idx);
			this._selectionFromKeyboard = false;
			return true;
		}
		return false;
	}

	/**
	 * Select a single file without modifying multi-select state. Used in tree
	 * mode where FM handles folder navigation and calls `onActive` on the newly
	 * focused file item — we just need to record the selection.
	 */
	selectSingle(change: TreeChange, index: number): void {
		// Do NOT set _selectionFromKeyboard here: FM has already moved the cursor
		// to this element and activated the outline via its own navigation path.
		// Setting the flag would trigger focusByElement in the $effect, which would
		// re-trigger onActive causing an infinite loop.
		this.idSelection.set(change.path, this.selectionId, index);
	}

	/** Handles arrow/vim navigation. Returns the index of the newly focused item, or undefined. */
	handleNavigation(e: KeyboardEvent): number | undefined {
		this._selectionFromKeyboard = true;
		const moved = this.updateKeyboardSelection(e);
		this._selectionFromKeyboard = false;
		if (moved) {
			const lastAdded = get(this.idSelection.getById(this.selectionId).lastAdded);
			return lastAdded?.index;
		}
		return undefined;
	}

	// ── Private helpers ──────────────────────────────────────────────────

	private updateKeyboardSelection(e: KeyboardEvent): boolean {
		const selectedFileIds = this.selectedFileIds;
		if (selectedFileIds.length === 0) return false;

		const files = this.changes;
		const filePathIndices = new Map(files.map((file, index) => [file.path, index]));

		const firstPath = selectedFileIds[0]!.path;
		const lastPath = selectedFileIds.at(-1)!.path;

		const topPath = files.find((f) => this.selectedPaths.has(f.path))?.path;
		const bottomPath = this.findBottomPath(files);

		const lastIdx = filePathIndices.get(lastPath) ?? -1;
		const firstIdx = filePathIndices.get(firstPath) ?? -1;
		let selectionDirection: "up" | "down" = firstIdx < lastIdx ? "down" : "up";

		const resolveAndApply = (id: string, offset: number, method: "add" | "set") => {
			const fileIndex = filePathIndices.get(id);
			if (fileIndex === undefined) return;
			const targetIndex = fileIndex + offset;
			const file = files[targetIndex];
			if (file) {
				this.idSelection[method](file.path, this.selectionId, targetIndex);
			}
		};

		switch (e.key) {
			case "a":
			case "A":
				if (e.metaKey || e.ctrlKey) {
					e.preventDefault();
					for (let i = 0; i < files.length; i++) {
						this.idSelection.add(files[i]!.path, this.selectionId, i);
					}
					this.idSelection.clearPreview(this.selectionId);
				}
				break;

			case "k":
			case "ArrowUp":
				e.preventDefault();
				if (e.shiftKey) {
					if (selectedFileIds.length === 1) {
						selectionDirection = "up";
					} else if (selectionDirection === "down") {
						this.idSelection.remove(lastPath, this.selectionId);
					}
					resolveAndApply(lastPath, -1, "add");
				} else {
					if (selectedFileIds.length > 1 && topPath !== undefined) {
						resolveAndApply(topPath, 0, "set");
					}
					if (selectedFileIds.length === 1) {
						resolveAndApply(firstPath, -1, "set");
					}
				}
				break;

			case "j":
			case "ArrowDown":
				e.preventDefault();
				if (e.shiftKey) {
					if (selectedFileIds.length === 1) {
						selectionDirection = "down";
					} else if (selectionDirection === "up") {
						this.idSelection.remove(lastPath, this.selectionId);
					}
					resolveAndApply(lastPath, 1, "add");
				} else {
					if (selectedFileIds.length > 1 && bottomPath !== undefined) {
						resolveAndApply(bottomPath, 0, "set");
					}
					if (selectedFileIds.length === 1) {
						resolveAndApply(firstPath, 1, "set");
					}
				}
				break;

			case "Escape":
				e.preventDefault();
				this.idSelection.clearPreview(this.selectionId);
				(e.currentTarget as HTMLElement).blur();
				return false;

			default:
				return false;
		}
		return true;
	}

	private findBottomPath(files: TreeChange[]): string | undefined {
		for (let i = files.length - 1; i >= 0; i--) {
			if (this.selectedPaths.has(files[i]!.path)) return files[i]!.path;
		}
		return undefined;
	}
}
