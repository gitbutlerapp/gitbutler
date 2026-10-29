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
import { syncUncommittedChanges } from "$lib/selection/syncUncommittedChanges.svelte";
import {
	UNCOMMITTED_SERVICE,
	type UncommittedService,
} from "$lib/selection/uncommittedService.svelte";
import { UI_STATE } from "$lib/state/uiState.svelte";
import { inject } from "@gitbutler/core/context";
import { getContext, setContext } from "svelte";
import type { FileSelectionManager } from "$lib/selection/fileSelectionManager.svelte";
import type { ProjectSettingsPageId } from "$lib/state/uiState.svelte";
import type { Worktree } from "@gitbutler/but-sdk";

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
	private getWorktree: () => Worktree | undefined;
	private uncommittedService = inject(UNCOMMITTED_SERVICE);

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
		/** Set for a linked worktree's lane, which has no stack id. */
		worktree?: () => Worktree | undefined;
	}) {
		this.uiState = inject(UI_STATE);
		this.fileSelection = inject(FILE_SELECTION_MANAGER);
		this.getProjectId = params.projectId;
		this.getStackId = params.stackId;
		this.getLaneId = params.laneId;
		this.getWorktree = params.worktree ?? (() => undefined);

		// The layout syncs the main checkout's changes; a worktree lane syncs its own.
		const worktreeName = this.getWorktree()?.name;
		if (worktreeName !== undefined) syncUncommittedChanges(params.projectId, worktreeName);

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

	/**
	 * The linked worktree this lane shows. Its files live in another checkout,
	 * so it never falls back to the main checkout's unassigned changes.
	 */
	get worktree(): Worktree | undefined {
		return this.getWorktree();
	}

	/**
	 * The branchless lane, which has neither a stack nor a worktree. A
	 * worktree's branches are written by name, so they need no stack id.
	 */
	get isReadOnly(): boolean {
		return !this.stackId && !this.worktree;
	}

	/** The tooltip of an action the lane can't take, since it is read-only. */
	get readOnlyTooltip(): string | undefined {
		if (this.isReadOnly) return "Read-only mode";
		return undefined;
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

	/** All selected commit IDs. Falls back to `[commitId]` when multi-select isn't active. */
	get selectedCommitIds(): string[] {
		const sel = this.selection.current;
		if (sel?.commitIds && sel.commitIds.length > 0) return sel.commitIds;
		if (sel?.commitId) return [sel.commitId];
		return [];
	}

	isCommitSelected(commitId: string): boolean {
		return this.selectedCommitIds.includes(commitId);
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
		return !!this.commitId;
	}

	get projectState() {
		return this.uiState.project(this.projectId);
	}

	get exclusiveAction() {
		return this.projectState.exclusiveAction.current;
	}

	get isCommitting(): boolean {
		const action = this.exclusiveAction;
		return (
			action?.type === "commit" &&
			action.stackId === this.stackId &&
			action.worktree === this.worktree?.name
		);
	}

	get dimmed(): boolean {
		return this.exclusiveAction?.type === "commit" && !this.isCommitting;
	}

	private get stackBusyState() {
		return this.projectState.stackBusy.current;
	}

	/** True when this stack is involved in a busy operation and should block interaction. */
	get stackBusy(): boolean {
		const stackIds = this.stackBusyState?.stackIds;
		return !!stackIds && !!this.stackId && stackIds.includes(this.stackId);
	}

	get busyCommitId(): string | undefined {
		return this.stackBusyState?.commitId;
	}

	get activeSelectionId(): SelectionId | undefined {
		if (this.commitId) {
			return this.commitSelection(this.commitId);
		} else if (this.branchName) {
			return this.branchSelection(this.branchName);
		}
		return this.uncommittedSelectionId;
	}

	/** The selection of a commit's files in this lane. */
	commitSelection(commitId: string): SelectionId {
		return createCommitSelection({
			commitId,
			stackId: this.stackId,
			worktree: this.worktree?.name,
		});
	}

	/** The selection of a branch's files in this lane. */
	branchSelection(branchName: string): SelectionId {
		return createBranchSelection({
			stackId: this.stackId,
			branchName,
			remote: undefined,
			worktree: this.worktree?.name,
		});
	}

	/** The uncommitted changes of this lane's checkout: its worktree's, or the main one's. */
	get uncommitted(): UncommittedService {
		return this.uncommittedService.at(this.worktree?.name);
	}

	/** The selection of this lane's uncommitted files. */
	get uncommittedSelectionId(): SelectionId {
		return createWorktreeSelection({ stackId: this.stackId, worktree: this.worktree?.name });
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
		return this.fileSelection.getById(this.uncommittedSelectionId);
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

	get isDetailsViewOpen(): boolean {
		return this.isSelectionPreviewOpen || this.hasStagedFileFocused;
	}

	closePreview(): void {
		if (this.activeSelectionId) {
			this.fileSelection.clear(this.activeSelectionId);
		}
		this.selection.set(undefined);
	}

	clearWorktreeSelection(): void {
		this.fileSelection.clear(this.uncommittedSelectionId);
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
