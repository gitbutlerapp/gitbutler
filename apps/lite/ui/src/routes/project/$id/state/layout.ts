export type Panel = "primary" | "show";
export const orderedPanels: Array<Panel> = ["primary", "show"];

export type ProjectLayoutState = {
	visiblePanels: Array<Panel>;
};

export const createInitialState = (): ProjectLayoutState => ({
	visiblePanels: [...orderedPanels],
});

export const initialState: ProjectLayoutState = createInitialState();

export const isPanelVisible = (state: ProjectLayoutState, panel: Panel): boolean =>
	state.visiblePanels.includes(panel);

export const showPanel = (state: ProjectLayoutState, panel: Panel) => {
	if (isPanelVisible(state, panel)) return;
	state.visiblePanels = orderedPanels.filter(
		(candidate) => candidate === panel || isPanelVisible(state, candidate),
	);
};

export const hidePanel = (state: ProjectLayoutState, panel: Panel) => {
	if (!isPanelVisible(state, panel)) return;

	state.visiblePanels = state.visiblePanels.filter((candidate) => candidate !== panel);
};

export const togglePanel = (state: ProjectLayoutState, panel: Panel) => {
	if (isPanelVisible(state, panel)) hidePanel(state, panel);
	else showPanel(state, panel);
};
