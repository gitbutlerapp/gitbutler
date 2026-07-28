import {
	branchFileParent,
	branchOperand,
	commitFileParent,
	commitOperand,
	fileOperand,
	hunkOperand,
	operandEquals,
	operandIdentityKey,
	type BranchOperand,
	type CommitOperand,
	type FileOperand,
	type HunkOperand,
	type Operand,
} from "#ui/operands.ts";
import type { Placement } from "#ui/operations/operation.ts";
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
import {
	resolveNavigationIndexSelection,
	type NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import type { SelectionScope } from "#ui/selection-scopes.ts";
import { createSelector } from "@reduxjs/toolkit";
import type { AbsorptionTarget } from "@gitbutler/but-sdk";
import { Match } from "effect";
import {
	branchesReducers,
	createInitialBranchesState,
	getBranchesSelectors,
	type BranchFilter,
	type BranchesState,
} from "./branches.ts";

export type SelectionState = {
	uncommittedFiles: string | null;
	outline: Operand | null;
	files: string | null;
	diff: HunkOperand | null;
};

type DetailsSelectionScope = Extract<SelectionScope, "uncommitted-files" | "outline">;

type CheckableOperand = Extract<Operand, { _tag: "Commit" | "File" }>;

type WorkspaceState = {
	checkedOperands: Record<string, CheckableOperand>;
	detailsSelectionScope: DetailsSelectionScope | null;
	highlightedCommitIds: Array<string>;
	mode: OutlineMode;
	selection: SelectionState;
};

const createInitialSelectionState = (): SelectionState => ({
	uncommittedFiles: null,
	outline: null,
	files: null,
	diff: null,
});

const createInitialWorkspaceState = (): WorkspaceState => ({
	checkedOperands: {},
	detailsSelectionScope: null,
	highlightedCommitIds: [],
	mode: defaultOutlineMode,
	selection: createInitialSelectionState(),
});

export type OutlineTab = "workspace" | "branches";

export type ProjectState = {
	filesVisible: boolean;
	outlineTab: OutlineTab;
	branches: BranchesState;
	workspace: WorkspaceState;
};

export const createInitialProjectState = (): ProjectState => ({
	filesVisible: false,
	outlineTab: "workspace",
	branches: createInitialBranchesState(),
	workspace: createInitialWorkspaceState(),
});

const hunkOperandIdentityKey = (operand: HunkOperand): string =>
	operandIdentityKey(hunkOperand(operand));

export const projectReducers = {
	setDetailsSelectionScope: (state: ProjectState, { scope }: { scope: DetailsSelectionScope }) => {
		state.workspace.detailsSelectionScope = scope;
	},
	selectUncommittedFiles: (state: ProjectState, { selection }: { selection: string | null }) => {
		const workspaceState = state.workspace;
		if (workspaceState.selection.uncommittedFiles === selection) return;

		workspaceState.selection.uncommittedFiles = selection;
	},
	selectOutline: (state: ProjectState, { selection }: { selection: Operand | null }) => {
		const workspaceState = state.workspace;
		if (
			selection &&
			workspaceState.selection.outline &&
			operandEquals(workspaceState.selection.outline, selection)
		)
			return;

		workspaceState.selection.outline = selection;
		workspaceState.selection.files = null;
		workspaceState.selection.diff = null;

		if (!selection || !isValidOutlineModeForSelection({ mode: workspaceState.mode, selection }))
			workspaceState.mode = defaultOutlineMode;
	},
	selectBranches: (state: ProjectState, { selection }: { selection: Operand | null }) => {
		branchesReducers.select(state.branches, { selection });
	},
	selectFiles: (state: ProjectState, { selection }: { selection: string | null }) => {
		const workspaceState = state.workspace;
		if (workspaceState.selection.files === selection) return;

		workspaceState.selection.files = selection;
	},
	selectDiff: (state: ProjectState, { selection }: { selection: HunkOperand | null }) => {
		const workspaceState = state.workspace;
		if (
			selection &&
			workspaceState.selection.diff &&
			operandEquals(hunkOperand(workspaceState.selection.diff), hunkOperand(selection))
		)
			return;

		workspaceState.selection.diff = selection;
	},
	startRewordCommit: (state: ProjectState, { commit }: { commit: CommitOperand }) => {
		const workspaceState = state.workspace;
		const selection = commitOperand(commit);
		if (
			!workspaceState.selection.outline ||
			!operandEquals(workspaceState.selection.outline, selection)
		) {
			workspaceState.selection.outline = selection;
			workspaceState.selection.files = null;
			workspaceState.selection.diff = null;
			if (!isValidOutlineModeForSelection({ mode: workspaceState.mode, selection }))
				workspaceState.mode = defaultOutlineMode;
		}

		workspaceState.mode = rewordCommitOutlineMode({ operand: commit });
	},
	startRenameBranch: (state: ProjectState, { branch }: { branch: BranchOperand }) => {
		const workspaceState = state.workspace;
		const selection = branchOperand(branch);
		if (
			!workspaceState.selection.outline ||
			!operandEquals(workspaceState.selection.outline, selection)
		) {
			workspaceState.selection.outline = selection;
			workspaceState.selection.files = null;
			workspaceState.selection.diff = null;
			if (!isValidOutlineModeForSelection({ mode: workspaceState.mode, selection }))
				workspaceState.mode = defaultOutlineMode;
		}

		workspaceState.mode = renameBranchOutlineMode({ operand: branch });
	},
	updateRewrittenBranchReferences: (
		state: ProjectState,
		{ oldBranch, newBranch }: { oldBranch: BranchOperand; newBranch: BranchOperand },
	) => {
		const workspaceState = state.workspace;
		const oldBranchOperand = branchOperand(oldBranch);
		const newBranchOperand = branchOperand(newBranch);

		if (
			workspaceState.selection.outline?._tag === "Branch" &&
			operandEquals(workspaceState.selection.outline, oldBranchOperand)
		)
			workspaceState.selection.outline = newBranchOperand;

		branchesReducers.updateRewrittenBranchReferences(state.branches, { oldBranch, newBranch });

		if (
			workspaceState.mode._tag === "RenameBranch" &&
			operandEquals(branchOperand(workspaceState.mode.operand), oldBranchOperand)
		)
			workspaceState.mode = renameBranchOutlineMode({ operand: newBranch });

		const oldFileParent = branchFileParent(oldBranch);
		const newFileParent = branchFileParent(newBranch);
		for (const [key, operand] of Object.entries(workspaceState.checkedOperands)) {
			if (
				operand._tag !== "File" ||
				operand.parent._tag !== "Branch" ||
				!operandEquals(operand.parent, oldFileParent)
			)
				continue;

			const newOperand = fileOperand({ parent: newFileParent, path: operand.path });
			delete workspaceState.checkedOperands[key];
			workspaceState.checkedOperands[operandIdentityKey(newOperand)] = newOperand;
		}
	},
	enterTransferMode: (state: ProjectState, { mode }: { mode: TransferMode }) => {
		state.workspace.mode = transferOutlineMode(mode);
	},
	enterKeyboardTransferMode: (
		state: ProjectState,
		{ sources, placement }: { sources: Array<Operand>; placement?: Placement },
	) => {
		const workspaceState = state.workspace;
		workspaceState.mode = transferOutlineMode(
			keyboardTransferMode({
				sources,
				placement: placement ?? "into",
				restoreSelection: {
					uncommittedFiles: workspaceState.selection.uncommittedFiles,
					outline: workspaceState.selection.outline,
					files: workspaceState.selection.files,
					diff: workspaceState.selection.diff,
				},
			}),
		);
	},
	enterAbsorbMode: (
		state: ProjectState,
		{ source, sourceTarget }: { source: Operand; sourceTarget: AbsorptionTarget },
	) => {
		const workspaceState = state.workspace;
		workspaceState.mode = absorbOutlineMode({
			source,
			restoreSelection: {
				uncommittedFiles: workspaceState.selection.uncommittedFiles,
				outline: workspaceState.selection.outline,
				files: workspaceState.selection.files,
				diff: workspaceState.selection.diff,
			},
			sourceTarget,
		});
	},
	updatePointerTransfer: (
		state: ProjectState,
		{ target, placement }: { target: Operand | null; placement: Placement | null },
	) => {
		const workspaceState = state.workspace;
		Match.value(workspaceState.mode).pipe(
			Match.when({ _tag: "Transfer", value: { _tag: "Pointer" } }, ({ value: mode }) => {
				const sameTarget =
					target === null
						? mode.target === null
						: mode.target !== null && operandEquals(mode.target, target);
				if (sameTarget && mode.placement === placement) return;

				workspaceState.mode = transferOutlineMode(
					pointerTransferMode({
						sources: mode.sources,
						target,
						placement,
					}),
				);
			}),
			Match.orElse(() => {}),
		);
	},
	updateTransferPlacement: (state: ProjectState, { placement }: { placement: Placement }) => {
		const workspaceState = state.workspace;
		Match.value(workspaceState.mode).pipe(
			Match.when({ _tag: "Transfer", value: { _tag: "Keyboard" } }, ({ value: mode }) => {
				workspaceState.mode = transferOutlineMode(
					keyboardTransferMode({
						sources: mode.sources,
						placement,
						restoreSelection: mode.restoreSelection,
					}),
				);
			}),
			Match.orElse(() => {}),
		);
	},
	exitMode: (state: ProjectState) => {
		state.workspace.mode = defaultOutlineMode;
	},
	cancelMode: (state: ProjectState) => {
		const workspaceState = state.workspace;
		const restoreSelection = Match.value(workspaceState.mode).pipe(
			Match.tags({
				Absorb: (mode) => mode.restoreSelection,
				Transfer: (mode) => (mode.value._tag === "Keyboard" ? mode.value.restoreSelection : null),
			}),
			Match.orElse(() => null),
		);
		workspaceState.mode = defaultOutlineMode;

		if (!restoreSelection) return;

		workspaceState.selection = restoreSelection;
	},
	setHighlightedCommitIds: (
		state: ProjectState,
		{ commitIds }: { commitIds: Array<string> | null },
	) => {
		state.workspace.highlightedCommitIds = commitIds ?? [];
	},
	checkOperand: (
		state: ProjectState,
		{ operand, checked }: { operand: CheckableOperand; checked: boolean },
	) => {
		const key = operandIdentityKey(operand);
		if (checked) state.workspace.checkedOperands[key] = operand;
		else delete state.workspace.checkedOperands[key];
	},
	checkOperands: (
		state: ProjectState,
		{ operands, checked }: { operands: Array<CheckableOperand>; checked: boolean },
	) => {
		for (const operand of operands) {
			const key = operandIdentityKey(operand);
			if (checked) state.workspace.checkedOperands[key] = operand;
			else delete state.workspace.checkedOperands[key];
		}
	},
	clearCheckedOperands: (state: ProjectState) => {
		state.workspace.checkedOperands = {};
	},
	updateRewrittenCommitReferences: (
		state: ProjectState,
		{ replacedCommits }: { replacedCommits: Record<string, string> },
	) => {
		const workspaceState = state.workspace;
		const selection = workspaceState.selection.outline;
		if (selection?._tag === "Commit") {
			const newId = replacedCommits[selection.commitId];
			if (newId !== undefined) {
				workspaceState.selection.outline = commitOperand({
					commitId: newId,
					changeId: selection.changeId,
				});
			}
		}

		branchesReducers.updateRewrittenCommitReferences(state.branches, { replacedCommits });

		for (const [key, operand] of Object.entries(workspaceState.checkedOperands)) {
			let newOperand: CheckableOperand | null = null;
			if (operand._tag === "Commit") {
				const newId = replacedCommits[operand.commitId];
				if (newId !== undefined)
					newOperand = commitOperand({ commitId: newId, changeId: operand.changeId });
			} else if (operand.parent._tag === "Commit") {
				const newId = replacedCommits[operand.parent.commitId];
				if (newId !== undefined) {
					newOperand = fileOperand({
						parent: commitFileParent({ commitId: newId, changeId: operand.parent.changeId }),
						path: operand.path,
					});
				}
			}
			if (!newOperand) continue;

			delete workspaceState.checkedOperands[key];
			workspaceState.checkedOperands[operandIdentityKey(newOperand)] = newOperand;
		}

		if (workspaceState.mode._tag === "RewordCommit") {
			const newId = replacedCommits[workspaceState.mode.operand.commitId];
			if (newId !== undefined) {
				workspaceState.mode = rewordCommitOutlineMode({
					operand: { commitId: newId, changeId: workspaceState.mode.operand.changeId },
				});
			}
		}
	},
	toggleFiles: (state: ProjectState) => {
		state.filesVisible = !state.filesVisible;
	},
	setOutlineTab: (state: ProjectState, { tab }: { tab: OutlineTab }) => {
		if (state.outlineTab === tab) return;

		state.outlineTab = tab;
		state.workspace.mode = defaultOutlineMode;
		// The branches tab has no uncommitted changes panel, so its selection
		// cannot drive the details pane. Leave the scope alone on the way back,
		// so returning to the workspace restores the panel it was showing.
		if (tab === "branches") state.workspace.detailsSelectionScope = "outline";
	},
	toggleBranchUnfolded: (state: ProjectState, { branchRef }: { branchRef: string }) => {
		branchesReducers.toggleUnfolded(state.branches, { branchRef });
	},
	setBranchSearch: (state: ProjectState, { search }: { search: string }) => {
		branchesReducers.setSearch(state.branches, { search });
	},
	toggleBranchFilter: (state: ProjectState, { filter }: { filter: BranchFilter }) => {
		branchesReducers.toggleFilter(state.branches, { filter });
	},
};

const selectCheckedOperands = createSelector(
	(state: ProjectState) => state.workspace.checkedOperands,
	(checkedOperands): Array<CheckableOperand> => Object.values(checkedOperands),
);

const selectCheckedOperandKeys = createSelector(
	(state: ProjectState) => state.workspace.checkedOperands,
	(checkedOperands): Set<string> => new Set(Object.keys(checkedOperands)),
);

type GroupedCheckedOperands = {
	commits: Array<CommitOperand>;
	files: Array<FileOperand>;
};

const selectGroupedCheckedOperands = createSelector(
	selectCheckedOperands,
	(checkedOperands): GroupedCheckedOperands =>
		checkedOperands.reduce<GroupedCheckedOperands>(
			(acc, operand) => {
				switch (operand._tag) {
					case "Commit":
						acc.commits.push(operand);
						break;
					case "File":
						acc.files.push(operand);
						break;
					default:
						operand satisfies never;
				}

				return acc;
			},
			{ commits: [], files: [] },
		),
);

const selectCheckedCommitIds = createSelector(
	selectGroupedCheckedOperands,
	(checkedGroupedOperands): Set<string> =>
		new Set(checkedGroupedOperands.commits.map((operand) => operand.commitId)),
);

const selectCheckedUncommittedFilePaths = createSelector(
	selectGroupedCheckedOperands,
	(checkedGroupedOperands): Set<string> =>
		new Set(
			checkedGroupedOperands.files.flatMap((operand) =>
				operand.parent._tag === "UncommittedChanges" ? [operand.path] : [],
			),
		),
);

const selectCheckedOperandCount = createSelector(
	selectCheckedOperands,
	(checkedOperands) => checkedOperands.length,
);

const selectHasCheckedOperands = createSelector(
	selectCheckedOperands,
	(checkedOperands) => checkedOperands.length > 0,
);

export const projectSelectors = {
	selectFilesVisible: (state: ProjectState) => state.filesVisible,
	selectOutlineTab: (state: ProjectState) => state.outlineTab,
	selectCanShowFiles: (state: ProjectState) =>
		state.workspace.detailsSelectionScope !== "uncommitted-files",
	selectDetailsSelectionScope: (state: ProjectState) => state.workspace.detailsSelectionScope,
	selectSelectionUncommittedFiles: (
		state: ProjectState,
		navigationIndex: NavigationIndex<string>,
	) =>
		resolveNavigationIndexSelection(
			navigationIndex,
			state.workspace.selection.uncommittedFiles,
			(path) => path,
		),
	selectIsSelectedOutline: (
		state: ProjectState,
		navigationIndex: NavigationIndex<Operand>,
		operand: Operand,
	) => {
		const selection = resolveNavigationIndexSelection(
			navigationIndex,
			state.workspace.selection.outline,
			operandIdentityKey,
		);
		return selection !== null && operandEquals(selection, operand);
	},
	/** The selection as stored, without resolving it against a navigation index. */
	selectPrimaryOutlineSelection: (state: ProjectState) => state.workspace.selection.outline,
	selectSelectionOutline: (state: ProjectState, navigationIndex: NavigationIndex<Operand>) =>
		resolveNavigationIndexSelection(
			navigationIndex,
			state.workspace.selection.outline,
			operandIdentityKey,
		),
	selectSelectionFiles: (state: ProjectState, navigationIndex: NavigationIndex<string>) =>
		resolveNavigationIndexSelection(
			navigationIndex,
			state.workspace.selection.files,
			(item) => item,
		),
	selectSelectionDiff: (state: ProjectState, navigationIndex: NavigationIndex<HunkOperand>) =>
		resolveNavigationIndexSelection(
			navigationIndex,
			state.workspace.selection.diff,
			hunkOperandIdentityKey,
		),
	selectOutlineModeState: (state: ProjectState) => state.workspace.mode,
	selectHighlightedCommitIds: (state: ProjectState) => state.workspace.highlightedCommitIds,
	selectOperandChecked: (state: ProjectState, operand: CheckableOperand) =>
		state.workspace.checkedOperands[operandIdentityKey(operand)] !== undefined,
	selectCheckedOperands,
	selectCheckedOperandKeys,
	selectCheckedCommitIds,
	selectCheckedUncommittedFilePaths,
	selectCheckedOperandCount,
	selectHasCheckedOperands,
	...getBranchesSelectors((state: ProjectState) => state.branches),
};
