import { createSlice, type PayloadAction } from '@reduxjs/toolkit';

type RecentlyPushedProjectIds = {
	recentlyPushedProjectIds: string[];
};

const slice = createSlice({
	name: 'recentProjects',
	initialState: { recentlyPushedProjectIds: [] } as RecentlyPushedProjectIds,
	reducers: {
		updateRecentlyPushedProjectIds(state, action: PayloadAction<string[]>) {
			state.recentlyPushedProjectIds = action.payload;
		}
	}
});

export const recentlyPushedProjectIdsReducer = slice.reducer;

export const { updateRecentlyPushedProjectIds } = slice.actions;
