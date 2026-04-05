import { type RefInfo, type WorktreeChanges } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { changesSectionItem, normalizeItem, type Item } from "../workspace/-Item.ts";

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

export const resolveSelectedWorkspaceItem = ({
	workspaceSelection,
	headInfo,
	worktreeChanges,
}: {
	workspaceSelection: WorkspaceSelectionState;
	headInfo: RefInfo;
	worktreeChanges: WorktreeChanges;
}): Item =>
	(workspaceSelection.item
		? normalizeItem(workspaceSelection.item, headInfo, worktreeChanges)
		: null) ?? changesSectionItem(null);
