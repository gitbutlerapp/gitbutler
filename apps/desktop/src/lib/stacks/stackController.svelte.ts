/**
 * Reactive controller that owns shared state for the StackView compound component.
 *
 * Manages selection coordination, preview state, and cross-panel communication
 * (e.g. left panel file click → right panel diff jump).
 *
 * Instantiate during component init so that `inject()` and `$effect()` bind
 * to the component lifecycle.
 */
import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
import {
	createBranchSelection,
	createCommitSelection,
	createWorktreeSelection,
	readKey,
	type SelectionId,
	type SelectedFile,
} from "$lib/selection/key";
import { UI_STATE } from "$lib/state/uiState.svelte";
import { inject } from "@gitbutler/core/context";
import { getContext, setContext } from "svelte";
import type { FileSelectionManager } from "$lib/selection/fileSelectionManager.svelte";
import type { ProjectSettingsPageId } from "$lib/state/uiState.svelte";

const STACK_CTX = Symbol("StackController");

/** Set the controller into Svelte component context. */
export function setStackContext(controller: StackController): void {
	setContext(STACK_CTX, controller);
}

/** Read the controller from Svelte component context. */
export function getStackContext(): StackController {
	const ctx = getContext<StackController>(STACK_CTX);
	if (!ctx) {
		throw new Error("StackController not found — wrap your component in a StackView");
	}
	return ctx;
}

export class StackController {
	private uiState;
	private fileSelection: FileSelectionManager;
	private getProjectId: () => string;
	private getStackId: () => string | undefined;
	private getLaneId: () => string;

	active = $state(false);
	visibleRange = $state<{ start: number; end: number } | undefined>();

	private diffJumpHandler?: (index: number) => void;
	private diffPopoutHandler?: () => void;

	private _focusedFile = $state<SelectedFile | undefined>();
	private _stagedFocusedFile = $state<SelectedFile | undefined>();

	constructor(params: {
		projectId: () => string;
		stackId: () => string | undefined;
		laneId: () => string;
	}) {
		this.uiState = inject(UI_STATE);
		this.fileSelection = inject(FILE_SELECTION_MANAGER);
		this.getProjectId = params.projectId;
		this.getStackId = params.stackId;
		this.getLaneId = params.laneId;

		$effect(() => {
			const store = this.focusedFileStore;
			if (!store) {
				this._focusedFile = undefined;
				return;
			}
			return store.subscribe((value) => {
				this._focusedFile = value?.key ? readKey(value.key) : undefined;
			});
		});

		$effect(() => {
			const store = this.stagedFocusedFileStore;
			if (!store) {
				this._stagedFocusedFile = undefined;
				return;
			}
			return store.subscribe((value) => {
				this._stagedFocusedFile = value?.key ? readKey(value.key) : undefined;
			});
		});
	}

	get projectId(): string {
		return this.getProjectId();
	}

	get stackId(): string | undefined {
		return this.getStackId();
	}

	get laneId(): string {
		return this.getLaneId();
	}

	get isReadOnly(): boolean {
		return !this.stackId;
	}

	get laneState() {
		return this.uiState.lane(this.laneId);
	}

	get selection() {
		return this.laneState.selection;
	}

	get commitId(): string | undefined {
		return this.selection.current?.commitId;
	}

	get branchName(): string | undefined {
		return this.selection.current?.branchName;
	}

	get upstream(): boolean | undefined {
		return this.selection.current?.upstream;
	}

	get previewOpen(): boolean | undefined {
		return this.selection.current?.previewOpen;
	}

	get isCommitView(): boolean {
		return !!(this.branchName && this.commitId);
	}

	get projectState() {
		return this.uiState.project(this.projectId);
	}

	get exclusiveAction() {
		return this.projectState.exclusiveAction.current;
	}

	get isCommitting(): boolean {
		return this.exclusiveAction?.type === "commit" && this.exclusiveAction.stackId === this.stackId;
	}

	get dimmed(): boolean {
		return (
			this.exclusiveAction?.type === "commit" && this.exclusiveAction?.stackId !== this.stackId
		);
	}

	get activeSelectionId(): SelectionId | undefined {
		if (this.commitId) {
			return createCommitSelection({ commitId: this.commitId, stackId: this.stackId });
		} else if (this.branchName) {
			return createBranchSelection({
				stackId: this.stackId,
				branchName: this.branchName,
				remote: undefined,
			});
		}
		return createWorktreeSelection({ stackId: this.stackId });
	}

	get focusedFileStore() {
		if (this.activeSelectionId) {
			return this.fileSelection.getById(this.activeSelectionId).lastAdded;
		}
		return undefined;
	}

	get focusedFile() {
		return this._focusedFile;
	}

	private get stagedFileGroup() {
		return this.fileSelection.getById(createWorktreeSelection({ stackId: this.stackId }));
	}

	get stagedFocusedFileStore() {
		return this.stagedFileGroup.lastAdded;
	}

	get stagedFocusedFile() {
		return this._stagedFocusedFile;
	}

	get hasPreviewTarget(): boolean {
		return !!(this.branchName || this.commitId || this.focusedFile);
	}

	get isSelectionPreviewOpen(): boolean {
		return this.hasPreviewTarget && !!this.previewOpen;
	}

	get hasStagedFileFocused(): boolean {
		return !!this.stagedFocusedFile;
	}

	get ircPanelOpen(): boolean {
		return this.selection.current?.irc === true;
	}

	get isDetailsViewOpen(): boolean {
		return this.isSelectionPreviewOpen || this.hasStagedFileFocused || this.ircPanelOpen;
	}

	closePreview(): void {
		if (this.activeSelectionId) {
			this.fileSelection.clear(this.activeSelectionId);
		}
		this.selection.set(undefined);
	}

	clearWorktreeSelection(): void {
		this.fileSelection.clear({ type: "worktree", stackId: this.stackId });
	}

	openProjectSettingsModal(selectedId?: ProjectSettingsPageId): void {
		this.uiState.global.modal.set({
			type: "project-settings",
			projectId: this.projectId,
			selectedId,
		});
	}

	registerDiffView(handlers: { jump: (index: number) => void; popout: () => void }): void {
		this.diffJumpHandler = handlers.jump;
		this.diffPopoutHandler = handlers.popout;
	}

	unregisterDiffView(): void {
		this.diffJumpHandler = undefined;
		this.diffPopoutHandler = undefined;
	}

	jumpToIndex(index: number): void {
		this.diffJumpHandler?.(index);
	}

	openFloatingDiff(): void {
		this.diffPopoutHandler?.();
	}

	setVisibleRange(range: { start: number; end: number } | undefined): void {
		this.visibleRange = range;
	}
}
