export type PanelsState = {
	filesVisible: boolean;
};

export const createInitialState = (): PanelsState => ({
	filesVisible: true,
});

export const initialState: PanelsState = createInitialState();

export const toggleFilesPanel = (state: PanelsState) => {
	state.filesVisible = !state.filesVisible;
};
