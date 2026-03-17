/**
 * Reactive controller that owns shared state for the StackView compound component.
 *
 * Manages selection coordination, preview state, and cross-panel communication
 * (e.g. left panel file click → right panel diff jump).
 *
 * Instantiate during component init so that `inject()` and `$effect()` bind
 * to the component lifecycle.
 */
import {
	createBranchSelection,
	createCommitSelection,
	createWorktreeSelection,
	readKey,
	type SelectionId,
} from "$lib/selection/key";
import { UI_STATE } from "$lib/state/uiState.svelte";
import { inject } from "@gitbutler/core/context";
import { getContext, setContext } from "svelte";
import { get } from "svelte/store";
import type { FileSelectionManager } from "$lib/selection/fileSelectionManager.svelte";
import type { ProjectSettingsPageId } from "$lib/settings/projectSettingsPages";

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
	/** Exposed for compound children that need direct uiState access (e.g. drop handlers). */
	uiState;
	private idSelection: FileSelectionManager;
	private getProjectId: () => string;
	private getStackId: () => string | undefined;
	private getLaneId: () => string;

	/** Whether this stack's focusable region is active. */
	active = $state(false);

	/** Visible range from MultiDiffView, consumed by WorktreeChanges for notching. */
	visibleRange = $state<{ start: number; end: number } | undefined>();

	/** Cross-panel diff view coordination. */
	private diffJumpHandler?: (index: number) => void;
	private diffPopoutHandler?: () => void;

	constructor(params: {
		projectId: () => string;
		stackId: () => string | undefined;
		laneId: () => string;
	}) {
		this.uiState = inject(UI_STATE);
		this.idSelection = inject(FILE_SELECTION_MANAGER);
		this.getProjectId = params.projectId;
		this.getStackId = params.stackId;
		this.getLaneId = params.laneId;
	}

	// ── Identity ──────────────────────────────────────────────────────

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

	// ── Active selection ID (for file selection tracking) ─────────────

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

	get activeLastAdded() {
		if (this.activeSelectionId) {
			return this.idSelection.getById(this.activeSelectionId).lastAdded;
		}
		return undefined;
	}

	get selectedFile() {
		const lastAdded = this.activeLastAdded;
		if (!lastAdded) return undefined;
		const value = get(lastAdded);
		return value?.key ? readKey(value.key) : undefined;
	}

	private get assignedSelection() {
		return this.idSelection.getById(createWorktreeSelection({ stackId: this.stackId }));
	}

	get lastAddedAssigned() {
		return this.assignedSelection.lastAdded;
	}

	get assignedKey() {
		const value = get(this.lastAddedAssigned);
		return value?.key ? readKey(value.key) : undefined;
	}

	get hasActiveSelection(): boolean {
		return !!(this.branchName || this.commitId || this.selectedFile);
	}

	get isPreviewOpenForSelection(): boolean {
		return this.hasActiveSelection && !!this.previewOpen;
	}

	get hasAssignedFiles(): boolean {
		return !!this.assignedKey;
	}

	get ircPanelOpen(): boolean {
		return this.selection.current?.irc === true;
	}

	get isDetailsViewOpen(): boolean {
		return this.isPreviewOpenForSelection || this.hasAssignedFiles || this.ircPanelOpen;
	}

	closePreview(): void {
		if (this.activeSelectionId) {
			this.idSelection.clear(this.activeSelectionId);
		}
		this.selection.set(undefined);
	}

	clearWorktreeSelection(): void {
		this.idSelection.clear({ type: "worktree", stackId: this.stackId });
	}

	// ── Cross-panel diff coordination ─────────────────────────────────

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
