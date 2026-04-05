import { Match } from "effect";

export type Panel = "primary" | "preview";

export type PanelLayout = { _tag: "Primary" } | { _tag: "Split"; focus: Panel };

export type ProjectLayoutState = {
	panelLayout: PanelLayout;
	isFullscreenPreviewOpen: boolean;
};

export type ProjectLayoutAction =
	| { _tag: "FocusPrimary" }
	| { _tag: "FocusPreview" }
	| { _tag: "TogglePreview" }
	| { _tag: "OpenFullscreenPreview" }
	| { _tag: "CloseFullscreenPreview" }
	| { _tag: "ToggleFullscreenPreview" };

export const initialProjectLayoutState: ProjectLayoutState = {
	panelLayout: { _tag: "Split", focus: "primary" },
	isFullscreenPreviewOpen: false,
};

export const projectLayoutReducer = (
	state: ProjectLayoutState,
	action: ProjectLayoutAction,
): ProjectLayoutState =>
	Match.value(action).pipe(
		Match.tagsExhaustive({
			FocusPrimary: (): ProjectLayoutState => ({
				...state,
				isFullscreenPreviewOpen: false,
				panelLayout:
					state.panelLayout._tag === "Primary"
						? state.panelLayout
						: { _tag: "Split", focus: "primary" },
			}),
			FocusPreview: (): ProjectLayoutState =>
				state.isFullscreenPreviewOpen
					? state
					: {
							...state,
							panelLayout: { _tag: "Split", focus: "preview" },
						},
			TogglePreview: (): ProjectLayoutState => ({
				...state,
				panelLayout:
					state.panelLayout._tag === "Primary"
						? { _tag: "Split", focus: "primary" }
						: { _tag: "Primary" },
			}),
			OpenFullscreenPreview: (): ProjectLayoutState => ({
				...state,
				isFullscreenPreviewOpen: true,
			}),
			CloseFullscreenPreview: (): ProjectLayoutState => ({
				...state,
				isFullscreenPreviewOpen: false,
			}),
			ToggleFullscreenPreview: (): ProjectLayoutState => ({
				...state,
				isFullscreenPreviewOpen: !state.isFullscreenPreviewOpen,
			}),
		}),
	);

export const isPreviewPanelVisible = (state: ProjectLayoutState): boolean =>
	state.panelLayout._tag === "Split";

const getPanelFocus = (state: ProjectLayoutState): Panel =>
	state.panelLayout._tag === "Split" ? state.panelLayout.focus : "primary";

export const getFocus = (state: ProjectLayoutState): Panel =>
	state.isFullscreenPreviewOpen ? "preview" : getPanelFocus(state);
