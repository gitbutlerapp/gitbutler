import { createContext, Dispatch, FC, ReactNode, useReducer } from "react";
import { Match } from "effect";

export type Panel = "primary" | "preview";

export type PanelLayout = { _tag: "Primary" } | { _tag: "Split"; focus: Panel };

export type WorkspaceLayoutState = {
	panelLayout: PanelLayout;
	isFullscreenPreviewOpen: boolean;
};

export type WorkspaceLayoutAction =
	| { _tag: "FocusPrimary" }
	| { _tag: "FocusPreview" }
	| { _tag: "TogglePreview" }
	| { _tag: "OpenFullscreenPreview" }
	| { _tag: "CloseFullscreenPreview" }
	| { _tag: "ToggleFullscreenPreview" };

const initialState: WorkspaceLayoutState = {
	panelLayout: { _tag: "Split", focus: "primary" },
	isFullscreenPreviewOpen: false,
};

const workspaceLayoutReducer = (
	state: WorkspaceLayoutState,
	action: WorkspaceLayoutAction,
): WorkspaceLayoutState =>
	Match.value(action).pipe(
		Match.tagsExhaustive({
			FocusPrimary: (): WorkspaceLayoutState => ({
				...state,
				isFullscreenPreviewOpen: false,
				panelLayout:
					state.panelLayout._tag === "Primary"
						? state.panelLayout
						: { _tag: "Split", focus: "primary" },
			}),
			FocusPreview: (): WorkspaceLayoutState =>
				state.isFullscreenPreviewOpen
					? state
					: {
							...state,
							panelLayout: { _tag: "Split", focus: "preview" },
						},
			TogglePreview: (): WorkspaceLayoutState => ({
				...state,
				panelLayout:
					state.panelLayout._tag === "Primary"
						? { _tag: "Split", focus: "primary" }
						: { _tag: "Primary" },
			}),
			OpenFullscreenPreview: (): WorkspaceLayoutState => ({
				...state,
				isFullscreenPreviewOpen: true,
			}),
			CloseFullscreenPreview: (): WorkspaceLayoutState => ({
				...state,
				isFullscreenPreviewOpen: false,
			}),
			ToggleFullscreenPreview: (): WorkspaceLayoutState => ({
				...state,
				isFullscreenPreviewOpen: !state.isFullscreenPreviewOpen,
			}),
		}),
	);

export const WorkspaceLayoutContext = createContext<
	[WorkspaceLayoutState, Dispatch<WorkspaceLayoutAction>] | null
>(null);

export const isPreviewPanelVisible = (state: WorkspaceLayoutState): boolean =>
	state.panelLayout._tag === "Split";

const getPanelFocus = (state: WorkspaceLayoutState): Panel =>
	state.panelLayout._tag === "Split" ? state.panelLayout.focus : "primary";

export const getFocus = (state: WorkspaceLayoutState): Panel =>
	state.isFullscreenPreviewOpen ? "preview" : getPanelFocus(state);

export const WorkspaceLayoutProvider: FC<{
	children: ReactNode;
}> = ({ children }) => {
	const state = useReducer(workspaceLayoutReducer, initialState);

	return <WorkspaceLayoutContext value={state}>{children}</WorkspaceLayoutContext>;
};
