import { configureStore } from "@reduxjs/toolkit";
import { setupListeners } from "@reduxjs/toolkit/query";
import { useDispatch, useSelector } from "react-redux";
import { liteApi } from "#ui/api/queries.ts";
import { projectSlice } from "#ui/projects/state.ts";

export const store = configureStore({
	reducer: {
		project: projectSlice.reducer,
		[liteApi.reducerPath]: liteApi.reducer,
	},
	middleware: (getDefaultMiddleware) =>
		getDefaultMiddleware({ serializableCheck: false }).concat(liteApi.middleware),
});

setupListeners(store.dispatch, (dispatch, { onFocus, onFocusLost }) => {
	const handleFocus = () => dispatch(onFocus());
	const handleBlur = () => dispatch(onFocusLost());
	window.addEventListener("focus", handleFocus);
	window.addEventListener("blur", handleBlur);

	return () => {
		window.removeEventListener("focus", handleFocus);
		window.removeEventListener("blur", handleBlur);
	};
});

type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;

export const useAppDispatch = useDispatch.withTypes<AppDispatch>();
export const useAppSelector = useSelector.withTypes<RootState>();
