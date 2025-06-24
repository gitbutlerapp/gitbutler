import { createSlice, type PayloadAction } from '@reduxjs/toolkit';

type RecentProjects = {
	recentlyInteractedProjectIds: string[];
};

const slice = createSlice({
	name: 'recentlyInteractedProjects',
	initialState: { recentlyInteractedProjectIds: [] } as RecentProjects,
	reducers: {
		updateRecentlyInteractedProjectIds(state, action: PayloadAction<string[]>) {
			state.recentlyInteractedProjectIds = action.payload;
		}
	}
});

export const recentlyInteractedProjectIdsReducer = slice.reducer;

export const { updateRecentlyInteractedProjectIds } = slice.actions;
