import { Match } from "effect";
import {
	initialProjectLayoutState,
	type ProjectLayoutAction,
	type ProjectLayoutState,
	projectLayoutReducer,
} from "./layout.ts";
import {
	initialWorkspaceSelectionState,
	type WorkspaceSelectionAction,
	type WorkspaceSelectionState,
	workspaceSelectionReducer,
} from "./selection.ts";

export type ProjectState = {
	layout: ProjectLayoutState;
	workspaceSelection: WorkspaceSelectionState;
};

export type ProjectStateAction = ProjectLayoutAction | WorkspaceSelectionAction;

export const initialProjectState: ProjectState = {
	layout: initialProjectLayoutState,
	workspaceSelection: initialWorkspaceSelectionState,
};

export const projectStateReducer = (
	state: ProjectState,
	action: ProjectStateAction,
): ProjectState =>
	Match.value(action).pipe(
		Match.tags({
			SelectItem: (action): ProjectState => ({
				layout: projectLayoutReducer(state.layout, { _tag: "FocusPrimary" }),
				workspaceSelection: workspaceSelectionReducer(state.workspaceSelection, action),
			}),
			SelectHunk: (action): ProjectState => ({
				...state,
				workspaceSelection: workspaceSelectionReducer(state.workspaceSelection, action),
			}),
		}),
		Match.orElse(
			(action): ProjectState => ({
				...state,
				layout: projectLayoutReducer(state.layout, action),
			}),
		),
	);
