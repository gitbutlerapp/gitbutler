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
import { selectFilesInList, updateSelection } from "$lib/selection/fileSelectionUtils";
import { type SelectionId } from "$lib/selection/key";
import { inject } from "@gitbutler/core/context";
import { FOCUS_MANAGER } from "@gitbutler/ui/focus/focusManager";
import { getContext, setContext, untrack } from "svelte";
import { get } from "svelte/store";
import type { TreeChange } from "$lib/hunks/change";
import type { FileSelectionManager } from "$lib/selection/fileSelectionManager.svelte";
import type { SelectedFile } from "$lib/selection/key";

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

	active = $state(false);

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

		const currentSelection = $derived(this.idSelection.getById(this.getSelectionId()));
		const lastAdded = $derived(currentSelection.lastAdded);
		const lastAddedIndex = $derived(get(lastAdded)?.index);

		$effect(() => {
			if (lastAddedIndex !== undefined) {
				untrack(() => {
					if (this.active) {
						this.focusManager.focusNthSibling(lastAddedIndex);
					}
				});
			}
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

	get selectedPaths(): Set<string> {
		return new Set(this.selectedFileIds.map((f) => f.path));
	}

	get hasSelectionInList(): boolean {
		return this.changes.some((change) => this.selectedPaths.has(change.path));
	}

	isSelected(path: string): boolean {
		return this.idSelection.has(path, this.selectionId);
	}

	select(e: MouseEvent | KeyboardEvent, change: TreeChange, index: number): void {
		selectFilesInList(
			e,
			change,
			this.changes,
			this.idSelection,
			true,
			index,
			this.selectionId,
			this.getAllowUnselect(),
		);
	}

	/** Returns true if the key was an activation key (Enter/Space/l) and select was called. */
	handleActivation(change: TreeChange, idx: number, e: KeyboardEvent): boolean {
		if (e.key === "Enter" || e.key === " " || e.key === "l") {
			e.stopPropagation();
			this.select(e, change, idx);
			return true;
		}
		return false;
	}

	/** Handles arrow/vim navigation. Returns the index of the newly focused item, or undefined. */
	handleNavigation(e: KeyboardEvent): number | undefined {
		if (
			updateSelection({
				allowMultiple: true,
				ctrlKey: e.ctrlKey,
				metaKey: e.metaKey,
				shiftKey: e.shiftKey,
				key: e.key,
				targetElement: e.currentTarget as HTMLElement,
				files: this.changes,
				selectedFileIds: this.selectedFileIds,
				fileIdSelection: this.idSelection,
				selectionId: this.selectionId,
				preventDefault: () => e.preventDefault(),
			})
		) {
			const lastAdded = get(this.idSelection.getById(this.selectionId).lastAdded);
			return lastAdded?.index;
		}
		return undefined;
	}
}
