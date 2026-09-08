import { configureStore, createListenerMiddleware } from "@reduxjs/toolkit";
import { useDispatch, useSelector, useStore } from "react-redux";
import { interfaceSlice } from "#ui/interface/state.ts";
import { writeStoredBranchFilters } from "#ui/projects/branches.ts";
import { projectSlice } from "#ui/projects/state.ts";

// Branch filters survive a reload (the slice seeds itself from what is stored),
// so each toggle writes the project's filters back as soon as it lands.
const persistBranchFilters = createListenerMiddleware();
persistBranchFilters.startListening({
	actionCreator: projectSlice.actions.toggleBranchFilter,
	effect: ({ payload: { projectId } }, api) => {
		writeStoredBranchFilters(
			projectId,
			projectSlice.selectors.selectBranchFilters(api.getState() as RootState, projectId),
		);
	},
});

export const store = configureStore({
	reducer: {
		interface: interfaceSlice.reducer,
		project: projectSlice.reducer,
	},
	middleware: (getDefaultMiddleware) =>
		getDefaultMiddleware().prepend(persistBranchFilters.middleware),
});

/* Without this a hot update rebuilds the store under still-mounted components,
   leaving two of them: measured 2 distinct stores, no reload. */
if (import.meta.hot) import.meta.hot.accept(() => window.location.reload());

type AppStore = typeof store;
type RootState = ReturnType<AppStore["getState"]>;
export type AppDispatch = AppStore["dispatch"];

export const useAppStore = useStore.withTypes<AppStore>();
export const useAppDispatch = useDispatch.withTypes<AppDispatch>();
export const useAppSelector = useSelector.withTypes<RootState>();
