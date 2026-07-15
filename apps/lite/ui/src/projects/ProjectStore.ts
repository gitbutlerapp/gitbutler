import { bytesEqual } from "#ui/api/bytes.ts";
import { rewrittenCommitOperand, rewrittenCommitSelection } from "#ui/commit.ts";
import {
	branchOperand,
	commitOperand,
	hunkOperand,
	operandEquals,
	operandIdentityKey,
	type BranchOperand,
	type CommitOperand,
	type HunkOperand,
	type Operand,
} from "#ui/operands.ts";
import type { OperationType } from "#ui/operations/operation.ts";
import {
	absorbOutlineMode,
	defaultOutlineMode,
	isValidOutlineModeForSelection,
	keyboardTransferMode,
	pointerTransferMode,
	renameBranchOutlineMode,
	rewordCommitOutlineMode,
	transferOutlineMode,
	type OutlineMode,
	type TransferMode,
} from "#ui/outline/mode.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import type { AbsorptionTarget, RefInfo, RelativeTo } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { makeAutoObservable, observable } from "mobx";

type Dialog =
	| { _tag: "None" }
	| { _tag: "ApplyBranchPicker" }
	| { _tag: "BranchPicker" }
	| { _tag: "CommandPalette" }
	| { _tag: "ProjectPicker" }
	| { _tag: "Settings" };

const resolveNavigationIndexSelection = <T>(
	navigationIndex: NavigationIndex<T>,
	selection: T | null,
	getKey: (item: T) => string,
): T | null =>
	selection !== null && navigationIndexIncludes(navigationIndex, selection, getKey)
		? selection
		: (navigationIndex.items[0] ?? null);

const hunkOperandIdentityKey = (operand: HunkOperand): string =>
	operandIdentityKey(hunkOperand(operand));

export class ProjectStore {
	detailsFullWindow = false;
	dialog: Dialog = { _tag: "None" };
	filesVisible = false;
	checkedCommitIds = observable.set<string>();
	commitTarget: RelativeTo | null = null;
	highlightedCommitIds: Array<string> = [];
	outlineMode: OutlineMode = defaultOutlineMode;
	outlineSelection: Operand | null = null;
	filesSelection: string | null = null;
	diffSelection: HunkOperand | null = null;

	constructor() {
		makeAutoObservable(
			this,
			{
				commitTarget: observable.ref,
				dialog: observable.ref,
				diffSelection: observable.ref,
				filesSelection: observable.ref,
				highlightedCommitIds: observable.ref,
				outlineMode: observable.ref,
				outlineSelection: observable.ref,
			},
			{ autoBind: true },
		);
	}

	get checkedCommitCount(): number {
		return this.checkedCommitIds.size;
	}

	get hasCheckedCommits(): boolean {
		return this.checkedCommitIds.size > 0;
	}

	selectOutline(selection: Operand | null) {
		if (selection && this.outlineSelection && operandEquals(this.outlineSelection, selection))
			return;

		this.outlineSelection = selection;
		this.filesSelection = null;
		this.diffSelection = null;

		if (!selection || !isValidOutlineModeForSelection({ mode: this.outlineMode, selection }))
			this.outlineMode = defaultOutlineMode;
	}

	selectFiles(selection: string | null) {
		if (this.filesSelection === selection) return;
		this.filesSelection = selection;
	}

	selectDiff(selection: HunkOperand | null) {
		if (
			selection &&
			this.diffSelection &&
			operandEquals(hunkOperand(this.diffSelection), hunkOperand(selection))
		)
			return;

		this.diffSelection = selection;
	}

	startRewordCommit(commit: CommitOperand) {
		const selection = commitOperand(commit);
		if (!this.outlineSelection || !operandEquals(this.outlineSelection, selection)) {
			this.outlineSelection = selection;
			this.filesSelection = null;
			this.diffSelection = null;
			if (!isValidOutlineModeForSelection({ mode: this.outlineMode, selection }))
				this.outlineMode = defaultOutlineMode;
		}

		this.outlineMode = rewordCommitOutlineMode({ operand: commit });
	}

	startRenameBranch(branch: BranchOperand) {
		const selection = branchOperand(branch);
		if (!this.outlineSelection || !operandEquals(this.outlineSelection, selection)) {
			this.outlineSelection = selection;
			this.filesSelection = null;
			this.diffSelection = null;
			if (!isValidOutlineModeForSelection({ mode: this.outlineMode, selection }))
				this.outlineMode = defaultOutlineMode;
		}

		this.outlineMode = renameBranchOutlineMode({ operand: branch });
	}

	updateRewrittenBranchReferences(oldBranch: BranchOperand, newBranch: BranchOperand) {
		const oldBranchOperand = branchOperand(oldBranch);
		const newBranchOperand = branchOperand(newBranch);

		if (
			this.outlineSelection?._tag === "Branch" &&
			operandEquals(this.outlineSelection, oldBranchOperand)
		)
			this.outlineSelection = newBranchOperand;

		if (
			this.commitTarget?.type === "referenceBytes" &&
			bytesEqual(this.commitTarget.subject, oldBranch.branchRef)
		) {
			this.commitTarget = {
				type: "referenceBytes",
				subject: newBranch.branchRef,
			};
		}

		if (
			this.outlineMode._tag === "RenameBranch" &&
			operandEquals(branchOperand(this.outlineMode.operand), oldBranchOperand)
		)
			this.outlineMode = renameBranchOutlineMode({ operand: newBranch });
	}

	enterTransferMode(mode: TransferMode) {
		this.outlineMode = transferOutlineMode(mode);
	}

	enterKeyboardTransferMode(source: Operand, operationType: OperationType = "into") {
		this.outlineMode = transferOutlineMode(
			keyboardTransferMode({
				source,
				operationType,
				restoreSelection: {
					outline: this.outlineSelection,
					files: this.filesSelection,
					diff: this.diffSelection,
				},
			}),
		);
	}

	enterAbsorbMode(source: Operand, sourceTarget: AbsorptionTarget) {
		this.outlineMode = absorbOutlineMode({
			source,
			restoreSelection: {
				outline: this.outlineSelection,
				files: this.filesSelection,
				diff: this.diffSelection,
			},
			sourceTarget,
		});
	}

	updatePointerTransfer(target: Operand | null, operationType: OperationType | null) {
		Match.value(this.outlineMode).pipe(
			Match.when({ _tag: "Transfer", value: { _tag: "Pointer" } }, ({ value: mode }) => {
				const sameTarget =
					target === null
						? mode.target === null
						: mode.target !== null && operandEquals(mode.target, target);
				if (sameTarget && mode.operationType === operationType) return;

				this.outlineMode = transferOutlineMode(
					pointerTransferMode({
						source: mode.source,
						target,
						operationType,
					}),
				);
			}),
			Match.orElse(() => {}),
		);
	}

	updateTransferOperationType(operationType: OperationType) {
		Match.value(this.outlineMode).pipe(
			Match.when({ _tag: "Transfer", value: { _tag: "Keyboard" } }, ({ value: mode }) => {
				this.outlineMode = transferOutlineMode(
					keyboardTransferMode({
						source: mode.source,
						operationType,
						restoreSelection: mode.restoreSelection,
					}),
				);
			}),
			Match.orElse(() => {}),
		);
	}

	exitMode() {
		this.outlineMode = defaultOutlineMode;
	}

	cancelMode() {
		const restoreSelection = Match.value(this.outlineMode).pipe(
			Match.tags({
				Absorb: (mode) => mode.restoreSelection,
				Transfer: (mode) => (mode.value._tag === "Keyboard" ? mode.value.restoreSelection : null),
			}),
			Match.orElse(() => null),
		);
		this.outlineMode = defaultOutlineMode;

		if (!restoreSelection) return;

		this.outlineSelection = restoreSelection.outline;
		this.filesSelection = restoreSelection.files;
		this.diffSelection = restoreSelection.diff;
	}

	setHighlightedCommitIds(commitIds: Array<string> | null) {
		this.highlightedCommitIds = commitIds ?? [];
	}

	setCommitChecked(commitId: string, checked: boolean) {
		if (checked) this.checkedCommitIds.add(commitId);
		else this.checkedCommitIds.delete(commitId);
	}

	setCommitsChecked(commitIds: Array<string>, checked: boolean) {
		for (const commitId of commitIds) {
			if (checked) this.checkedCommitIds.add(commitId);
			else this.checkedCommitIds.delete(commitId);
		}
	}

	clearCheckedCommits() {
		this.checkedCommitIds.clear();
	}

	setCommitTarget(commitTarget: RelativeTo | null) {
		this.commitTarget = commitTarget;
	}

	updateRewrittenCommitReferences(replacedCommits: Record<string, string>, headInfo: RefInfo) {
		const commit = rewrittenCommitSelection({
			selection: this.outlineSelection,
			replacedCommits,
			headInfo,
		});
		if (commit) this.outlineSelection = commit;

		if (this.commitTarget?.type === "commit") {
			const commitId = replacedCommits[this.commitTarget.subject];
			if (commitId !== undefined) this.commitTarget = { type: "commit", subject: commitId };
		}

		const oldIds = new Set(this.checkedCommitIds);
		for (const oldId of oldIds) {
			const newId = replacedCommits[oldId];
			if (newId !== undefined) {
				this.checkedCommitIds.delete(oldId);
				this.checkedCommitIds.add(newId);
			}
		}

		if (this.outlineMode._tag === "RewordCommit") {
			const commit = rewrittenCommitOperand({
				commit: this.outlineMode.operand,
				replacedCommits,
				headInfo,
			});
			if (commit) this.outlineMode = rewordCommitOutlineMode({ operand: commit });
		}
	}

	toggleFiles() {
		this.filesVisible = !this.filesVisible;
	}

	setDetailsFullWindow(fullWindow: boolean) {
		this.detailsFullWindow = fullWindow;
	}

	toggleDetailsFullWindow() {
		this.detailsFullWindow = !this.detailsFullWindow;
	}

	openDialog(dialog: Dialog) {
		this.dialog = dialog;
	}

	closeDialog() {
		this.dialog = { _tag: "None" };
	}

	isOutlineSelected(navigationIndex: NavigationIndex<Operand>, operand: Operand): boolean {
		const selection = this.selectedOutline(navigationIndex);
		return selection !== null && operandEquals(selection, operand);
	}

	selectedOutline(navigationIndex: NavigationIndex<Operand>): Operand | null {
		return resolveNavigationIndexSelection(
			navigationIndex,
			this.outlineSelection,
			operandIdentityKey,
		);
	}

	selectedFiles(navigationIndex: NavigationIndex<string>): string | null {
		return resolveNavigationIndexSelection(navigationIndex, this.filesSelection, (item) => item);
	}

	selectedDiff(navigationIndex: NavigationIndex<HunkOperand>): HunkOperand | null {
		return resolveNavigationIndexSelection(
			navigationIndex,
			this.diffSelection,
			hunkOperandIdentityKey,
		);
	}

	isCommitChecked(commitId: string): boolean {
		return this.checkedCommitIds.has(commitId);
	}
}
