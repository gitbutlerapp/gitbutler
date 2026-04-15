import {
	branchItem,
	commitItem,
	itemEquals,
	type BranchItem,
	type CommitItem,
	type Item,
} from "../workspace/Item.ts";
import { type OperationSource } from "../workspace/OperationSource.ts";
import {
	defaultWorkspaceMode,
	moveOperationMode,
	renameBranchWorkspaceMode,
	rewordCommitWorkspaceMode,
	rubOperationMode,
	type WorkspaceMode,
} from "../workspace/WorkspaceMode.ts";

export type WorkspaceSelectionState = {
	hunk: string | null;
	item: Item | null;
};

const createInitialWorkspaceSelectionState = (): WorkspaceSelectionState => ({
	hunk: null,
	item: null,
});

export type WorkspaceState = {
	expandedCommitId: string | null;
	highlightedCommitIds: Array<string>;
	mode: WorkspaceMode;
	selection: WorkspaceSelectionState;
};

export const createInitialState = (): WorkspaceState => ({
	expandedCommitId: null,
	highlightedCommitIds: [],
	mode: defaultWorkspaceMode,
	selection: createInitialWorkspaceSelectionState(),
});

export const initialState: WorkspaceState = createInitialState();

const normalizeModeForSelectedItem = (mode: WorkspaceMode, item: Item | null) =>
	(mode._tag === "RewordCommit" &&
		(item === null || item._tag !== "Commit" || item.commitId !== mode.commitId)) ||
	(mode._tag === "RenameBranch" &&
		(item === null ||
			!itemEquals(
				item,
				branchItem({
					stackId: mode.stackId,
					branchRef: mode.branchRef,
				}),
			)))
		? defaultWorkspaceMode
		: mode;

export const closeCommitFiles = (state: WorkspaceState, item: CommitItem) => {
	state.expandedCommitId = null;
	selectItem(state, commitItem(item));
};

export const enterMoveMode = (state: WorkspaceState, source: OperationSource) => {
	state.mode = moveOperationMode({ source });
};

export const enterRubMode = (state: WorkspaceState, source: OperationSource) => {
	state.mode = rubOperationMode({ source });
};

export const exitMode = (state: WorkspaceState) => {
	state.mode = defaultWorkspaceMode;
};

export const openCommitFiles = (state: WorkspaceState, item: CommitItem) => {
	state.expandedCommitId = item.commitId;
	selectItem(state, commitItem(item));
};

export const selectHunk = (state: WorkspaceState, hunk: string | null) => {
	state.selection.hunk = hunk;
};

export const selectItem = (state: WorkspaceState, item: Item | null) => {
	state.selection.item = item;
	state.selection.hunk = null;
	state.mode = normalizeModeForSelectedItem(state.mode, item);
};

export const setExpandedCommitId = (state: WorkspaceState, commitId: string | null) => {
	state.expandedCommitId = commitId;
};

export const setHighlightedCommitIds = (state: WorkspaceState, commitIds: Array<string> | null) => {
	state.highlightedCommitIds = commitIds ?? [];
};

export const startRenameBranch = (state: WorkspaceState, item: BranchItem) => {
	selectItem(state, branchItem(item));
	state.mode = renameBranchWorkspaceMode({
		stackId: item.stackId,
		branchRef: item.branchRef,
	});
};

export const startRewordCommit = (state: WorkspaceState, item: CommitItem) => {
	selectItem(state, commitItem(item));
	state.mode = rewordCommitWorkspaceMode({ commitId: item.commitId });
};

export const toggleCommitFiles = (state: WorkspaceState, item: CommitItem) => {
	if (state.expandedCommitId === item.commitId) {
		closeCommitFiles(state, item);
		return;
	}

	openCommitFiles(state, item);
};

const selectSelection = (state: WorkspaceState): WorkspaceSelectionState => state.selection;

export const selectSelectedItem = (state: WorkspaceState): Item | null =>
	selectSelection(state).item;

export const selectSelectedHunk = (state: WorkspaceState): string | null =>
	selectSelection(state).hunk;

export const selectMode = (state: WorkspaceState): WorkspaceMode => state.mode;

export const selectExpandedCommitId = (state: WorkspaceState): string | null =>
	state.expandedCommitId;

export const selectHighlightedCommitIds = (state: WorkspaceState): Array<string> =>
	state.highlightedCommitIds;

export const normalizeSelectedHunk = ({
	hunkKeys,
	selectedHunk,
}: {
	hunkKeys: Array<string>;
	selectedHunk: string | null;
}): string | undefined => {
	if (selectedHunk !== null && hunkKeys.includes(selectedHunk)) return selectedHunk;
	return hunkKeys[0];
};
