import { createBackendApi, type BackendApi } from "$lib/state/backendApi";
import { uiStateSlice } from "$lib/state/uiState.svelte";
import { InjectionToken } from "@gitbutler/core/context";
import { mergeUnlisten } from "@gitbutler/ui/utils/mergeUnlisten";
import { combineSlices, configureStore, type Slice } from "@reduxjs/toolkit";
import { setupListeners, type RootState } from "@reduxjs/toolkit/query";
import { FLUSH, PAUSE, PERSIST, persistReducer, PURGE, REGISTER, REHYDRATE } from "redux-persist";
import persistStore from "redux-persist/lib/persistStore";
import storage from "redux-persist/lib/storage";
import type { IBackend } from "$lib/backend";
import type { PostHogWrapper } from "$lib/telemetry/posthog";

export const CLIENT_STATE = new InjectionToken<ClientState>("ClientState");

type StoreResult = ReturnType<typeof createStore>;
type AppStore = StoreResult["store"];
type AppState = ReturnType<AppStore["getState"]>;
export type AppDispatch = AppStore["dispatch"];

/**
 * A redux store with dependency injection through middleware.
 */
export class ClientState {
	private store: AppStore;
	private reducer: StoreResult["reducer"];
	readonly dispatch: typeof this.store.dispatch;

	rootState = $state.raw<AppState | undefined>(undefined);
	readonly uiState = $derived(this.rootState?.uiState);

	/** rtk-query api for communicating with the back end. */
	readonly backendApi: BackendApi;

	constructor(backend: IBackend, posthog: PostHogWrapper) {
		const ctx = {
			getState: () => this.rootState as unknown as RootState<any, any, any>,
			getDispatch: () => this.dispatch,
			posthog,
		};
		this.backendApi = createBackendApi(ctx);

		const { store, reducer } = createStore({
			backend,
			backendApi: this.backendApi,
		});

		this.store = store;
		this.reducer = reducer;
		this.dispatch = this.store.dispatch;
		this.rootState = this.store.getState();

		$effect(() =>
			mergeUnlisten(
				this.store.subscribe(() => {
					this.rootState = this.store.getState();
				}),
				setupListeners(this.store.dispatch),
			),
		);
	}

	initPersist(): Promise<void> {
		return new Promise<void>((resolve) => {
			persistStore(this.store, undefined, () => resolve());
		});
	}

	injectPersistedSlice<S>(slice: Slice<S>): () => S | undefined {
		this.reducer.inject(
			{
				reducerPath: slice.reducerPath,
				reducer: persistReducer({ key: slice.reducerPath, storage }, slice.reducer),
			},
			{ overrideExisting: false },
		);
		return () => {
			const state = this.rootState as Record<string, unknown> | undefined;
			if (state && slice.reducerPath in state) {
				return state[slice.reducerPath] as S;
			}
			return undefined;
		};
	}
}

function createStore(params: { backend: IBackend; backendApi: BackendApi }) {
	const { backend, backendApi } = params;
	const reducer = combineSlices(backendApi).inject({
		reducerPath: uiStateSlice.reducerPath,
		reducer: persistReducer(
			{ key: uiStateSlice.reducerPath, storage: storage },
			uiStateSlice.reducer,
		),
	});
	const store = configureStore({
		reducer,
		middleware: (getDefaultMiddleware) => {
			return getDefaultMiddleware({
				thunk: { extraArgument: { backend } },
				serializableCheck: {
					ignoredActions: [FLUSH, REHYDRATE, PAUSE, PERSIST, PURGE, REGISTER],
					ignoredPaths: ["backend"],
				},
			}).concat(backendApi.middleware);
		},
	});

	return { store, reducer };
}
