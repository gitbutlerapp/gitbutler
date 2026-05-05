import { allPanels, type Panel } from "#ui/panels.ts";

export type PanelsState = {
	visiblePanels: Array<Panel>;
};

export const createInitialState = (): PanelsState => ({
	visiblePanels: allPanels,
});

export const initialState: PanelsState = createInitialState();

export const isPanelVisible = (state: PanelsState, panel: Panel): boolean =>
	state.visiblePanels.includes(panel);

export const showPanel = (state: PanelsState, panel: Panel) => {
	if (isPanelVisible(state, panel)) return;
	state.visiblePanels = [...state.visiblePanels, panel];
};

export const hidePanel = (state: PanelsState, panel: Panel) => {
	if (!isPanelVisible(state, panel)) return;

	state.visiblePanels = state.visiblePanels.filter((candidate) => candidate !== panel);
};

export const togglePanel = (state: PanelsState, panel: Panel) => {
	if (isPanelVisible(state, panel)) hidePanel(state, panel);
	else showPanel(state, panel);
};
