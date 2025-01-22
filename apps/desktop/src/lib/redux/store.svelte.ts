import { reduxApi } from './api';
import { configureStore } from '@reduxjs/toolkit';
import type { Tauri } from '$lib/backend/tauri';

export class DesktopRedux {
	readonly store: ReturnType<typeof createStore>;
	readonly dispatch: typeof DesktopRedux.prototype.store.dispatch;

	constructor(readonly tauri: Tauri) {
		this.store = createStore(tauri);
		this.dispatch = this.store.dispatch;
		this.rootState$ = this.store.getState();

		$effect(() =>
			this.store.subscribe(() => {
				this.rootState$ = this.store.getState();
			})
		);
	}

	rootState$ = $state({} as ReturnType<typeof this.store.getState>);
}

/**
 * We need this function in order to declare the store type in `DesktopState`
 * and then assign the value in the constructor.
 */
function createStore(tauri: Tauri) {
	return configureStore({
		reducer: {
			api: reduxApi.reducer
		},
		middleware: (getDefaultMiddleware) => {
			return getDefaultMiddleware({
				thunk: { extraArgument: { tauri } }
			}).concat(reduxApi.middleware);
		}
	});
}
