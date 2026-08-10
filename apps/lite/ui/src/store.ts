import { configureStore } from "@reduxjs/toolkit";
import { useDispatch, useSelector, useStore } from "react-redux";
import { interfaceSlice } from "#ui/interface/state.ts";
import { projectSlice } from "#ui/projects/state.ts";

export const store = configureStore({
	reducer: {
		interface: interfaceSlice.reducer,
		project: projectSlice.reducer,
	},
});

type AppStore = typeof store;
type RootState = ReturnType<AppStore["getState"]>;
export type AppDispatch = AppStore["dispatch"];

export const useAppStore = useStore.withTypes<AppStore>();
export const useAppDispatch = useDispatch.withTypes<AppDispatch>();
export const useAppSelector = useSelector.withTypes<RootState>();
