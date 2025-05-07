import { createSlice, type PayloadAction } from '@reduxjs/toolkit';

export type SidebarTab = 'projects' | 'reviews';

type SidebarState = {
	/** The sidebar open state is only relevant on mobile or tablet */
	open: boolean;
	currentTab: SidebarTab;
};

const initialState: SidebarState = { open: false, currentTab: 'projects' };

const dashboardSidebarSlice = createSlice({
	name: 'dashboardSidebar',
	initialState,
	reducers: {
		toggle(state) {
			state.open = !state.open;
		},
		close(state) {
			state.open = false;
		},
		open(state) {
			state.open = true;
		},
		setTab(state, action: PayloadAction<SidebarTab>) {
			state.currentTab = action.payload;
		}
	}
});

export const {
	toggle: dashboardSidebarToggle,
	close: dashboardSidebarClose,
	open: dashboardSidebarOpen,
	setTab: dashboardSidebarSetTab
} = dashboardSidebarSlice.actions;
export const dashboardSidebarReducer = dashboardSidebarSlice.reducer;
