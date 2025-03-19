import { tauriBaseQuery } from './backendQuery';
import { butlerModule } from './butlerModule';
import { ReduxTag } from './tags';
import { uiStatePersistConfig, uiStateSlice } from './uiState.svelte';
import { changeSelectionSlice } from '$lib/selection/changeSelection.svelte';
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { buildCreateApi, coreModule, type RootState } from '@reduxjs/toolkit/query';
import { FLUSH, PAUSE, PERSIST, persistReducer, PURGE, REGISTER, REHYDRATE } from 'redux-persist';
import persistStore from 'redux-persist/lib/persistStore';
import type { Tauri } from '$lib/backend/tauri';
import type { GitHubClient } from '$lib/forge/github/githubClient';

/**
 * GitHub API object that enables the declaration and usage of endpoints
 * colocated with the feature they support.
 */
export type GitHubApi = ReturnType<typeof createGitHubApi>;

/**
 * A redux store with dependency injection through middleware.
 */
export class ClientState {
	private store: ReturnType<typeof createStore>;
	readonly dispatch: typeof this.store.dispatch;

	// $state requires field declaration, but we have to assign the initial
	// value in the constructor such that we can inject dependencies. The
	// incorrect casting `as` seems difficult to avoid.
	rootState = $state.raw({} as ReturnType<typeof this.store.getState>);
	readonly changeSelection = $derived(this.rootState.changeSelection);
	readonly uiState = $derived(this.rootState.uiState);

	/** rtk-query api for communicating with the back end. */
	readonly backendApi: ReturnType<typeof createBackendApi>;

	/** rtk-query api for communicating with GitHub. */
	readonly githubApi: ReturnType<typeof createGitHubApi>;

	constructor(tauri: Tauri, github: GitHubClient) {
		const butlerMod = butlerModule({
			// Reactive loop without nested function.
			// TODO: Can it be done without nesting?
			getState: () => () => this.rootState as any as RootState<any, any, any>,
			getDispatch: () => this.dispatch
		});
		this.githubApi = createGitHubApi(butlerMod);
		this.backendApi = createBackendApi(butlerMod);
		this.store = createStore(tauri, github, this.backendApi, this.githubApi);
		this.dispatch = this.store.dispatch;
		this.rootState = this.store.getState();

		$effect(() =>
			this.store.subscribe(() => {
				this.rootState = this.store.getState();
			})
		);
	}
}

/**
 * We need this function in order to declare the store type in `DesktopState`
 * and then assign the value in the constructor.
 */
function createStore(
	tauri: Tauri,
	gitHubClient: GitHubClient,
	backendApi: ReturnType<typeof createBackendApi>,
	githubApi: ReturnType<typeof createGitHubApi>
) {
	const reducer = combineReducers({
		// RTK Query API for the back end.
		[backendApi.reducerPath]: backendApi.reducer,
		[githubApi.reducerPath]: githubApi.reducer,
		// File and hunk selection state.
		[changeSelectionSlice.reducerPath]: changeSelectionSlice.reducer,
		[uiStateSlice.reducerPath]: persistReducer(uiStatePersistConfig, uiStateSlice.reducer)
	});

	const store = configureStore({
		reducer: reducer,
		middleware: (getDefaultMiddleware) => {
			return getDefaultMiddleware({
				thunk: { extraArgument: { tauri, gitHubClient } },
				serializableCheck: {
					ignoredActions: [FLUSH, REHYDRATE, PAUSE, PERSIST, PURGE, REGISTER]
				}
			}).concat(backendApi.middleware, githubApi.middleware);
		}
	});
	persistStore(store);
	return store;
}

/**
 * Creates an rtk-query API object with extended endpoint methods.
 *
 * Inspired by the react hooks bundled with rtk we want to enable an API
 * that does not require any handling state in services. In said hooks
 * the state and dispatcher are acquired from the application context.
 * Unlike with React, it isn't possible to access the Svelte context
 * during event handling.
 */
export function createBackendApi(butlerMod: ReturnType<typeof butlerModule>) {
	return buildCreateApi(
		coreModule(),
		butlerMod
	)({
		reducerPath: 'backend',
		tagTypes: Object.values(ReduxTag),
		baseQuery: tauriBaseQuery,
		endpoints: (_) => {
			return {};
		}
	});
}

export function createGitHubApi(butlerMod: ReturnType<typeof butlerModule>) {
	return buildCreateApi(
		coreModule(),
		butlerMod
	)({
		reducerPath: 'github',
		tagTypes: Object.values(ReduxTag),
		baseQuery: tauriBaseQuery,
		endpoints: (_) => {
			return {};
		}
	});
}
