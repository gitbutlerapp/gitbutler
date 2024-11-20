import { exampleReducer } from '$lib/redux/example';
import { postReducer } from '$lib/redux/posts/slice';
import { configureStore } from '@reduxjs/toolkit';

/**
 * The base store.
 *
 * This is a low level API and should not be used directly.
 * @private
 */
export const _store = configureStore({
	reducer: {
		example: exampleReducer,
		post: postReducer
	}
});

export type RootState = ReturnType<typeof _store.getState>;
export type AppDispatch = typeof _store.dispatch;
