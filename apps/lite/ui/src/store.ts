import { configureStore } from "@reduxjs/toolkit";
import { useDispatch, useSelector } from "react-redux";
import { projectReducer } from "#ui/projects/state.ts";
import { commandsReducer } from "#ui/commands/state.ts";

export const store = configureStore({
	reducer: {
		commands: commandsReducer,
		project: projectReducer,
	},
});

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;

export const useAppDispatch = useDispatch.withTypes<AppDispatch>();
export const useAppSelector = useSelector.withTypes<RootState>();
