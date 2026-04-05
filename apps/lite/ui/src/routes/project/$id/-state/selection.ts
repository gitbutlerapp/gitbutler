import { getCommonBaseCommitId } from "#ui/domain/RefInfo.ts";
import { type RefInfo, type WorktreeChanges } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { type Item } from "../workspace/-Item.ts";

export type WorkspaceSelectionState = {
	item: Item | null;
	file: string | null;
	hunk: string | null;
};

export type WorkspaceSelectionAction =
	| { _tag: "SelectItem"; item: Item | null }
	| { _tag: "SelectFile"; file: string | null }
	| { _tag: "SelectHunk"; hunk: string | null };

export const initialWorkspaceSelectionState: WorkspaceSelectionState = {
	item: null,
	file: null,
	hunk: null,
};

export const workspaceSelectionReducer = (
	state: WorkspaceSelectionState,
	action: WorkspaceSelectionAction,
): WorkspaceSelectionState =>
	Match.value(action).pipe(
		Match.tagsExhaustive({
			SelectItem: ({ item }): WorkspaceSelectionState => ({
				item,
				file: null,
				hunk: null,
			}),
			SelectFile: ({ file }): WorkspaceSelectionState => ({
				...state,
				file,
				hunk: null,
			}),
			SelectHunk: ({ hunk }): WorkspaceSelectionState => ({
				...state,
				hunk,
			}),
		}),
	);

const normalizeSelectedItem = (
	item: Item,
	headInfo: RefInfo,
	worktreeChanges: WorktreeChanges,
): Item | null =>
	Match.value(item).pipe(
		Match.tag("Changes", (item) => item),
		Match.tag("Change", (item) => {
			if (!worktreeChanges.changes.find((change) => change.path === item.path)) return null;
			if (
				!worktreeChanges.assignments.find(
					(assignment) => assignment.stackId === item.stackId && assignment.path === item.path,
				)
			)
				return null;
			return item;
		}),
		Match.tag("Segment", (item) => {
			const stack = headInfo.stacks.find((stack) => stack.id !== null && stack.id === item.stackId);
			if (!stack) return null;
			const segment = stack.segments[item.segmentIndex];
			if (!segment) return null;
			const branchName = segment.refName?.displayName ?? null;
			if (branchName !== item.branchName) return null;
			return item;
		}),
		Match.tag("Commit", (item) => {
			const stack = headInfo.stacks.find((stack) => stack.id !== null && stack.id === item.stackId);
			if (!stack) return null;
			const segment = stack.segments[item.segmentIndex];
			if (!segment) return null;
			if (!segment.commits.some((commit) => commit.id === item.commitId)) return null;
			return item;
		}),
		Match.tag("BaseCommit", (item) => {
			const commonBaseCommitId = getCommonBaseCommitId(headInfo);
			return commonBaseCommitId === item.commitId ? item : null;
		}),
		Match.exhaustive,
	);

export const resolveSelectedWorkspaceItem = ({
	workspaceSelection,
	worktreeChanges,
	headInfo,
	defaultItem,
}: {
	workspaceSelection: WorkspaceSelectionState;
	worktreeChanges: WorktreeChanges;
	headInfo: RefInfo;
	defaultItem: Item;
}): Item =>
	(workspaceSelection.item
		? normalizeSelectedItem(workspaceSelection.item, headInfo, worktreeChanges)
		: null) ?? defaultItem;

export const normalizeSelectedFile = ({
	paths,
	selectedFile,
}: {
	paths: Array<string>;
	selectedFile: string | null | undefined;
}): string | undefined => {
	if (selectedFile != null && paths.includes(selectedFile)) return selectedFile;
	return paths[0];
};

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
