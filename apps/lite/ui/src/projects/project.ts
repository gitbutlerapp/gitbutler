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
	type FileParent,
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
import { decodeBytes } from "#ui/api/bytes.ts";
import {
	createInitialUpstreamState,
	getUpstreamSelectors,
	upstreamReducers,
	type UpstreamState,
} from "./upstream.ts";

export type SelectionState = {
	uncommittedFiles: string | null;
	outline: Operand | null;
	files: string | null;
	diff: HunkOperand | null;
};

type DetailsSelectionScope = Extract<SelectionScope, "uncommitted-files" | "outline">;

type CheckableOperand = Extract<Operand, { _tag: "Commit" | "File" }>;

export type BranchTab = "diff" | "pr";

type WorkspaceState = {
	checkedOperands: Record<string, CheckableOperand>;
	detailsSelectionScope: DetailsSelectionScope | null;
	/**
	 * Branch segments whose commits are hidden, keyed by full ref name.
	 *
	 * Folded rather than unfolded, the inverse of the branches tab: the
	 * workspace is the working view, so its commits show by default and it is
	 * hiding them that is the exception worth recording.
	 */
	foldedSegments: Record<string, true>;
	highlightedCommitIds: Array<string>;
	mode: OutlineMode;
	selectedBranchTabs: Record<string, BranchTab>;
	selection: SelectionState;
	/**
	 * File filter queries, or `null` while a filter is closed. An open but empty
	 * filter is not the same as a closed one: it keeps the input in place and the
	 * list unnarrowed.
	 *
	 * The outline's uncommitted list and the details pane's file list filter
	 * independently, and can both be open at once.
	 */
	uncommittedFilesFilter: string | null;
	filesFilter: string | null;
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
	foldedSegments: {},
	highlightedCommitIds: [],
	mode: defaultOutlineMode,
	selectedBranchTabs: {},
	selection: createInitialSelectionState(),
	uncommittedFilesFilter: null,
	filesFilter: null,
});

export type OutlineTab = "workspace" | "upstream" | "branches";

const defaultBranchTab: BranchTab = "diff";

export type ProjectState = {
	filesVisible: boolean;
	outlineTab: OutlineTab;
	branches: BranchesState;
	upstream: UpstreamState;
	workspace: WorkspaceState;
};

export const createInitialProjectState = (): ProjectState => ({
	filesVisible: false,
	outlineTab: "workspace",
	branches: createInitialBranchesState(),
	upstream: createInitialUpstreamState(),
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
	selectUpstream: (state: ProjectState, { selection }: { selection: Operand | null }) => {
		upstreamReducers.select(state.upstream, { selection });
	},
	toggleUpstreamSegment: (state: ProjectState, { segmentId }: { segmentId: string }) => {
		upstreamReducers.toggleSegment(state.upstream, { segmentId });
	},
	toggleUpstreamIncoming: (state: ProjectState) => {
		upstreamReducers.toggleIncoming(state.upstream);
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
	setSelectedBranchTab: (
		state: ProjectState,
		{ branchName, tab }: { branchName: string; tab: BranchTab },
	) => {
		if (state.workspace.selectedBranchTabs[branchName] === tab) return;

		state.workspace.selectedBranchTabs[branchName] = tab;
	},
	setOutlineTab: (state: ProjectState, { tab }: { tab: OutlineTab }) => {
		if (state.outlineTab === tab) return;

		state.outlineTab = tab;
		state.workspace.mode = defaultOutlineMode;
		// The branches and upstream tabs have no uncommitted changes panel, so
		// their selection cannot drive the details pane. Leave the scope alone on
		// the way back, so returning to the workspace restores the panel it was
		// showing.
		if (tab !== "workspace") state.workspace.detailsSelectionScope = "outline";
	},
	toggleSegmentFolded: (state: ProjectState, { branchRef }: { branchRef: string }) => {
		if (state.workspace.foldedSegments[branchRef]) delete state.workspace.foldedSegments[branchRef];
		else state.workspace.foldedSegments[branchRef] = true;
	},
	/**
	 * Folds or unfolds several segments at once, for acting on a whole stack.
	 * Toggling each of them instead would invert a partly folded stack rather
	 * than bring it to one state.
	 */
	setSegmentsFolded: (
		state: ProjectState,
		{ branchRefs, folded }: { branchRefs: Array<string>; folded: boolean },
	) => {
		for (const branchRef of branchRefs) {
			if (folded) state.workspace.foldedSegments[branchRef] = true;
			else delete state.workspace.foldedSegments[branchRef];
		}
	},
	toggleBranchUnfolded: (state: ProjectState, { branchRef }: { branchRef: string }) => {
		branchesReducers.toggleUnfolded(state.branches, { branchRef });
	},
	setBranchesUnfolded: (
		state: ProjectState,
		{ branchRefs, unfolded }: { branchRefs: Array<string>; unfolded: boolean },
	) => {
		branchesReducers.setUnfolded(state.branches, { branchRefs, unfolded });
	},
	/** Pass `null` to close the filter, which also clears the query. */
	setUncommittedFilesFilter: (state: ProjectState, { filter }: { filter: string | null }) => {
		const workspaceState = state.workspace;
		if (workspaceState.uncommittedFilesFilter === filter) return;

		workspaceState.uncommittedFilesFilter = filter;
	},
	/** Pass `null` to close the filter, which also clears the query. */
	setFilesFilter: (state: ProjectState, { filter }: { filter: string | null }) => {
		const workspaceState = state.workspace;
		if (workspaceState.filesFilter === filter) return;

		workspaceState.filesFilter = filter;
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
	uncommittedFiles: Array<FileOperand>;
	filesByCommitId: Map<string, Array<FileOperand>>;
	filesByBranchRef: Map<string, Array<FileOperand>>;
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
					case "File": {
						switch (operand.parent._tag) {
							case "UncommittedChanges":
								acc.uncommittedFiles.push(operand);
								break;
							case "Commit":
								acc.filesByCommitId.getOrInsert(operand.parent.commitId, []).push(operand);
								break;
							case "Branch":
								acc.filesByBranchRef
									.getOrInsert(decodeBytes(operand.parent.branchRef), [])
									.push(operand);
								break;
							default:
								operand.parent satisfies never;
						}
						break;
					}
					default:
						operand satisfies never;
				}

				return acc;
			},
			{
				commits: [],
				uncommittedFiles: [],
				filesByCommitId: new Map(),
				filesByBranchRef: new Map(),
			},
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
		new Set(checkedGroupedOperands.uncommittedFiles.map((operand) => operand.path)),
);

const selectCheckedOperandCount = createSelector(
	selectCheckedOperands,
	(checkedOperands) => checkedOperands.length,
);

export const projectSelectors = {
	selectFilesVisible: (state: ProjectState) => state.filesVisible,
	selectOutlineTab: (state: ProjectState) => state.outlineTab,
	selectBranchTab: (state: ProjectState, branchName: string): BranchTab =>
		state.workspace.selectedBranchTabs[branchName] ?? defaultBranchTab,
	selectCanShowFiles: (state: ProjectState) =>
		state.workspace.detailsSelectionScope !== "uncommitted-files",
	selectDetailsSelectionScope: (state: ProjectState) => state.workspace.detailsSelectionScope,
	selectUncommittedFilesFilter: (state: ProjectState) => state.workspace.uncommittedFilesFilter,
	selectFilesFilter: (state: ProjectState) => state.workspace.filesFilter,
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
	selectFoldedSegments: (state: ProjectState) => state.workspace.foldedSegments,
	selectSegmentFolded: (state: ProjectState, branchRef: string) =>
		state.workspace.foldedSegments[branchRef] === true,
	selectHighlightedCommitIds: (state: ProjectState) => state.workspace.highlightedCommitIds,
	selectOperandChecked: (state: ProjectState, operand: CheckableOperand) =>
		state.workspace.checkedOperands[operandIdentityKey(operand)] !== undefined,
	selectCheckedOperands,
	selectCheckedOperandKeys,
	selectCheckedCommitIds,
	selectCheckedUncommittedFilePaths,
	selectCheckedOperandCount,
	// Checking has been defined in a flexible way to support heterogeneous items, however in the UI
	// we currently only allow a single context of checked items at a time, hence these selectors.
	selectCheckedOperandsContext: (state: ProjectState): CheckableOperand["_tag"] | null =>
		selectCheckedOperandCount(state) === 0
			? null
			: selectGroupedCheckedOperands(state).commits.length > 0
				? "Commit"
				: "File",
	selectCanCheckCommits: (state: ProjectState) =>
		selectCheckedOperands(state).length === selectGroupedCheckedOperands(state).commits.length,
	selectCanCheckFiles: (state: ProjectState, fileParent: FileParent) => {
		switch (fileParent._tag) {
			case "UncommittedChanges":
				return (
					selectCheckedOperands(state).length ===
					selectGroupedCheckedOperands(state).uncommittedFiles.length
				);
			case "Commit":
				return (
					selectCheckedOperands(state).length ===
					(selectGroupedCheckedOperands(state).filesByCommitId.get(fileParent.commitId)?.length ??
						0)
				);
			// We currently don't support any operations on branch files.
			case "Branch":
				return false;
		}
	},
	...getBranchesSelectors((state: ProjectState) => state.branches),
	...getUpstreamSelectors((state: ProjectState) => state.upstream),
};
