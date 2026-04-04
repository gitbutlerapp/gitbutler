import { type RefInfo, type WorktreeChanges } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { normalizeItem, type Item } from "../workspace/-Item.ts";

export type WorkspaceSelectionState = {
	item: Item | null;
	hunk: string | null;
};

export type WorkspaceSelectionAction =
	| { _tag: "SelectItem"; item: Item | null }
	| { _tag: "SelectHunk"; hunk: string | null };

export const initialWorkspaceSelectionState: WorkspaceSelectionState = {
	item: null,
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
	navigationModelItems,
}: {
	workspaceSelection: WorkspaceSelectionState;
	headInfo: RefInfo;
	worktreeChanges: WorktreeChanges;
	navigationModelItems: Array<Item>;
}): Item | null =>
	(workspaceSelection.item
		? normalizeItem(workspaceSelection.item, headInfo, worktreeChanges)
		: null) ??
	navigationModelItems[0] ??
	null;
