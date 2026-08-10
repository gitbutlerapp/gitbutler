import {
	branchFileParent,
	branchOperand,
	commitFileParent,
	commitOperand,
	hunkOperand,
	fileOperand,
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
	keyboardTransferMode,
	pointerTransferMode,
	renameBranchOutlineMode,
	rewordCommitOutlineMode,
	transferOutlineMode,
	type OutlineMode,
	type TransferMode,
} from "#ui/outline/mode.ts";
import {
	cursorKey,
	remapDiffCursor,
	remapDiffCursorBranch,
	type WorkspaceCursorSnapshot,
} from "#ui/cursors.ts";
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

/** The workspace page's two lists; the one named here drives the details pane. */
export type WorkspaceList = "stacks" | "uncommitted";

type CheckableOperand = Extract<Operand, { _tag: "Commit" | "File" | "Hunk" }>;

export type BranchTab = "diff" | "pr";

/**
 * A conflict checked for a batch resolution. Ids survive the rewrites that
 * compact hunk positions, so checks carry across; the commit id is remapped.
 */
type CheckedConflict = { commitId: string; path: string; id: string };

const conflictCheckKey = ({ commitId, path, id }: CheckedConflict): string =>
	`${commitId}\u0000${path}\u0000${id}`;

type WorkspaceState = {
	checkedOperands: Record<string, CheckableOperand>;
	checkedConflicts: Record<string, CheckedConflict>;
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
	/**
	 * The diff cursor. Its five siblings live in the URL (use-cursor.ts); this
	 * one's identity is the exact selected line groups, which no legible param
	 * carries, so it stays here behind the same seam.
	 */
	diffCursor: HunkOperand | null;
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
	/**
	 * Directories whose contents the tree view hides, keyed by directory path.
	 *
	 * Collapsed rather than expanded, as with {@link WorkspaceState.foldedSegments}:
	 * a tree opens showing everything, so it is the hiding that is worth recording.
	 * One set per list, as with the filters — the two lists hold different files
	 * and each is somewhere the user is looking separately.
	 */
	uncommittedFilesCollapsedDirectories: Record<string, true>;
	filesCollapsedDirectories: Record<string, true>;
};

const createInitialWorkspaceState = (): WorkspaceState => ({
	checkedOperands: {},
	checkedConflicts: {},
	foldedSegments: {},
	highlightedCommitIds: [],
	mode: defaultOutlineMode,
	selectedBranchTabs: {},
	diffCursor: null,
	uncommittedFilesFilter: null,
	filesFilter: null,
	uncommittedFilesCollapsedDirectories: {},
	filesCollapsedDirectories: {},
});

export type OutlineTab = "workspace" | "upstream" | "branches";

const defaultBranchTab: BranchTab = "diff";

export type ProjectState = {
	filesVisible: boolean;
	branches: BranchesState;
	upstream: UpstreamState;
	workspace: WorkspaceState;
};

export const createInitialProjectState = (): ProjectState => ({
	filesVisible: false,
	branches: createInitialBranchesState(),
	upstream: createInitialUpstreamState(),
	workspace: createInitialWorkspaceState(),
});

export const projectReducers = {
	selectDiffCursor: (state: ProjectState, { selection }: { selection: HunkOperand | null }) => {
		const current = state.workspace.diffCursor;
		if (
			selection !== null &&
			current !== null &&
			cursorKey.diff(current) === cursorKey.diff(selection)
		)
			return;

		state.workspace.diffCursor = selection;
	},
	toggleUpstreamSegment: (state: ProjectState, { segmentId }: { segmentId: string }) => {
		upstreamReducers.toggleSegment(state.upstream, { segmentId });
	},
	startRewordCommit: (state: ProjectState, { commit }: { commit: CommitOperand }) => {
		state.workspace.mode = rewordCommitOutlineMode({ operand: commit });
	},
	startRenameBranch: (state: ProjectState, { branch }: { branch: BranchOperand }) => {
		state.workspace.mode = renameBranchOutlineMode({ operand: branch });
	},
	updateRewrittenBranchReferences: (
		state: ProjectState,
		{ oldBranch, newBranch }: { oldBranch: BranchOperand; newBranch: BranchOperand },
	) => {
		const workspaceState = state.workspace;
		const oldBranchOperand = branchOperand(oldBranch);

		if (workspaceState.diffCursor) {
			workspaceState.diffCursor = remapDiffCursorBranch(
				workspaceState.diffCursor,
				oldBranch,
				newBranch,
			);
		}

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
		{
			sources,
			placement,
			restoreSelection,
		}: {
			sources: Array<Operand>;
			placement?: Placement;
			restoreSelection: WorkspaceCursorSnapshot;
		},
	) => {
		state.workspace.mode = transferOutlineMode(
			keyboardTransferMode({ sources, placement: placement ?? "into", restoreSelection }),
		);
	},
	enterAbsorbMode: (
		state: ProjectState,
		{
			source,
			sourceTarget,
			restoreSelection,
		}: {
			source: Operand;
			sourceTarget: AbsorptionTarget;
			restoreSelection: WorkspaceCursorSnapshot;
		},
	) => {
		state.workspace.mode = absorbOutlineMode({ source, restoreSelection, sourceTarget });
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
	checkConflict: (
		state: ProjectState,
		{ conflict, checked }: { conflict: CheckedConflict; checked: boolean },
	) => {
		const key = conflictCheckKey(conflict);
		if (checked) state.workspace.checkedConflicts[key] = conflict;
		else delete state.workspace.checkedConflicts[key];
	},
	clearCheckedConflicts: (state: ProjectState) => {
		if (Object.keys(state.workspace.checkedConflicts).length === 0) return;
		state.workspace.checkedConflicts = {};
	},
	updateRewrittenCommitReferences: (
		state: ProjectState,
		{ replacedCommits }: { replacedCommits: Record<string, string> },
	) => {
		const workspaceState = state.workspace;

		if (workspaceState.diffCursor)
			workspaceState.diffCursor = remapDiffCursor(workspaceState.diffCursor, replacedCommits);

		for (const [key, conflict] of Object.entries(workspaceState.checkedConflicts)) {
			const newId = replacedCommits[conflict.commitId];
			if (newId === undefined) continue;
			delete workspaceState.checkedConflicts[key];
			const moved = { ...conflict, commitId: newId };
			workspaceState.checkedConflicts[conflictCheckKey(moved)] = moved;
		}

		for (const [key, operand] of Object.entries(workspaceState.checkedOperands)) {
			let newOperand: CheckableOperand | null = null;
			if (operand._tag === "Commit") {
				const newId = replacedCommits[operand.commitId];
				if (newId !== undefined)
					newOperand = commitOperand({ commitId: newId, changeId: operand.changeId });
			} else if (operand._tag === "File" && operand.parent._tag === "Commit") {
				const newId = replacedCommits[operand.parent.commitId];
				if (newId !== undefined) {
					newOperand = fileOperand({
						parent: commitFileParent({ commitId: newId, changeId: operand.parent.changeId }),
						path: operand.path,
					});
				}
			} else if (operand._tag === "Hunk" && operand.parent.parent._tag === "Commit") {
				const newId = replacedCommits[operand.parent.parent.commitId];
				if (newId !== undefined) {
					newOperand = hunkOperand({
						...operand,
						parent: {
							...operand.parent,
							parent: commitFileParent({
								commitId: newId,
								changeId: operand.parent.parent.changeId,
							}),
						},
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
	toggleUncommittedFilesDirectoryCollapsed: (state: ProjectState, { path }: { path: string }) => {
		const collapsed = state.workspace.uncommittedFilesCollapsedDirectories;
		if (collapsed[path]) delete collapsed[path];
		else collapsed[path] = true;
	},
	toggleFilesDirectoryCollapsed: (state: ProjectState, { path }: { path: string }) => {
		const collapsed = state.workspace.filesCollapsedDirectories;
		if (collapsed[path]) delete collapsed[path];
		else collapsed[path] = true;
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

/** The checks belonging to `commitId`, so a different commit reads as none. */
const selectCheckedConflictsFor = createSelector(
	(state: ProjectState) => state.workspace.checkedConflicts,
	(_state: ProjectState, commitId: string) => commitId,
	(checkedConflicts, commitId): Array<CheckedConflict> =>
		Object.values(checkedConflicts).filter((conflict) => conflict.commitId === commitId),
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
	hunksByFileParent: Map<string, Array<HunkOperand>>;
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
					case "Hunk": {
						const parentKey = operandIdentityKey(operand.parent.parent);
						acc.hunksByFileParent.getOrInsert(parentKey, []).push(operand);
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
				hunksByFileParent: new Map(),
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
	selectBranchTab: (state: ProjectState, branchName: string): BranchTab =>
		state.workspace.selectedBranchTabs[branchName] ?? defaultBranchTab,

	selectUncommittedFilesFilter: (state: ProjectState) => state.workspace.uncommittedFilesFilter,
	selectFilesFilter: (state: ProjectState) => state.workspace.filesFilter,
	selectUncommittedFilesCollapsedDirectories: (state: ProjectState) =>
		state.workspace.uncommittedFilesCollapsedDirectories,
	selectFilesCollapsedDirectories: (state: ProjectState) =>
		state.workspace.filesCollapsedDirectories,
	/** The diff cursor as stored; its siblings live in the URL. */
	selectDiffCursor: (state: ProjectState) => state.workspace.diffCursor,
	/** A primitive, so checking one conflict re-renders one card. */
	selectIsConflictChecked: (state: ProjectState, conflict: CheckedConflict): boolean =>
		conflictCheckKey(conflict) in state.workspace.checkedConflicts,
	selectCheckedConflicts: selectCheckedConflictsFor,
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
				: selectGroupedCheckedOperands(state).hunksByFileParent.size > 0
					? "Hunk"
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
	selectCanCheckHunks: (state: ProjectState, fileParent: FileParent) => {
		// We currently don't support any operations on branch hunks.
		if (fileParent._tag === "Branch") return false;

		return (
			selectCheckedOperands(state).length ===
			(selectGroupedCheckedOperands(state).hunksByFileParent.get(operandIdentityKey(fileParent))
				?.length ?? 0)
		);
	},
	...getBranchesSelectors((state: ProjectState) => state.branches),
	...getUpstreamSelectors((state: ProjectState) => state.upstream),
};
