import { changeSelectionSlice } from '$lib/selection/changeSelection.svelte';
import { tauriBaseQuery } from '$lib/state/backendQuery';
import { butlerModule } from '$lib/state/butlerModule';
import { ReduxTag } from '$lib/state/tags';
import { uiStatePersistConfig, uiStateSlice } from '$lib/state/uiState.svelte';
import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';
import { combineSlices, configureStore, type Reducer } from '@reduxjs/toolkit';
import { buildCreateApi, coreModule, setupListeners, type RootState } from '@reduxjs/toolkit/query';
import { FLUSH, PAUSE, PERSIST, persistReducer, PURGE, REGISTER, REHYDRATE } from 'redux-persist';
import persistStore from 'redux-persist/lib/persistStore';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { Tauri } from '$lib/backend/tauri';
import type { GitHubClient } from '$lib/forge/github/githubClient';
import type { GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
import type { IrcClient } from '$lib/irc/ircClient.svelte';

/**
 * GitHub API object that enables the declaration and usage of endpoints
 * colocated with the feature they support.
 */
export type BackendApi = ReturnType<typeof createBackendApi>;

/**
 * GitHub API object that enables the declaration and usage of endpoints
 * colocated with the feature they support.
 */
export type GitHubApi = ReturnType<typeof createGitHubApi>;

/**
 * GitLab API object that enables the declaration and usage of endpoints
 * colocated with the feature they support.
 */
export type GitLabApi = ReturnType<typeof createGitLabApi>;

/**
 * A redux store with dependency injection through middleware.
 */
export class ClientState {
	private store: ReturnType<typeof createStore>['store'];
	private reducer: ReturnType<typeof createStore>['reducer'];
	readonly dispatch: typeof this.store.dispatch;

	// $state requires field declaration, but we have to assign the initial
	// value in the constructor such that we can inject dependencies. The
	// incorrect casting `as` seems difficult to avoid.
	rootState = $state.raw({} as ReturnType<typeof this.store.getState>);
	readonly changeSelection = $derived(this.rootState.changeSelection);
	readonly uiState = $derived(this.rootState.uiState);

	/** rtk-query api for communicating with the back end. */
	readonly backendApi: BackendApi;

	/** rtk-query api for communicating with GitHub. */
	readonly githubApi: GitHubApi;

	/** rtk-query api for communicating with GitLab. */
	readonly gitlabApi: GitLabApi;

	get reactiveState() {
		return this.rootState;
	}

	constructor(
		tauri: Tauri,
		gitHubClient: GitHubClient,
		gitLabClient: GitLabClient,
		ircClient: IrcClient,
		posthog: PostHogWrapper
	) {
		const butlerMod = butlerModule({
			// Reactive loop without nested function.
			// TODO: Can it be done without nesting?
			getState: () => () => this.rootState as any as RootState<any, any, any>,
			getDispatch: () => this.dispatch
		});
		this.githubApi = createGitHubApi(butlerMod);
		this.gitlabApi = createGitLabApi(butlerMod);
		this.backendApi = createBackendApi(butlerMod);

		const { store, reducer } = createStore({
			tauri,
			gitHubClient,
			gitLabClient,
			ircClient,
			backendApi: this.backendApi,
			githubApi: this.githubApi,
			gitlabApi: this.gitlabApi,
			posthog
		});

		this.store = store;
		this.reducer = reducer;
		setupListeners(this.store.dispatch);
		this.dispatch = this.store.dispatch;
		this.rootState = this.store.getState();

		$effect(() =>
			mergeUnlisten(
				this.store.subscribe(() => {
					this.rootState = this.store.getState();
				}),
				setupListeners(this.store.dispatch)
			)
		);
	}

	inject(reducerPath: string, reducer: Reducer<any>) {
		return this.reducer.inject({ reducerPath, reducer }, { overrideExisting: false });
	}

	initPersist() {
		persistStore(this.store);
	}
}

/**
 * We need this function in order to declare the store type in `DesktopState`
 * and then assign the value in the constructor.
 */
function createStore(params: {
	tauri: Tauri;
	gitHubClient: GitHubClient;
	gitLabClient: GitLabClient;
	ircClient: IrcClient;
	backendApi: BackendApi;
	githubApi: GitHubApi;
	gitlabApi: GitLabApi;
	posthog: PostHogWrapper;
}) {
	const {
		tauri,
		gitHubClient,
		gitLabClient,
		ircClient,
		backendApi,
		githubApi,
		gitlabApi,
		posthog
	} = params;
	const reducer = combineSlices(
		// RTK Query API for the back end.
		backendApi,
		githubApi,
		gitlabApi,
		changeSelectionSlice
	);
	const reducer2 = reducer.inject({
		reducerPath: uiStateSlice.reducerPath,
		reducer: persistReducer(uiStatePersistConfig, uiStateSlice.reducer)
	});

	const store = configureStore({
		reducer: reducer2,
		middleware: (getDefaultMiddleware) => {
			return getDefaultMiddleware({
				thunk: {
					extraArgument: { tauri, gitHubClient, gitLabClient, ircClient, posthog }
				},
				serializableCheck: {
					ignoredActions: [FLUSH, REHYDRATE, PAUSE, PERSIST, PURGE, REGISTER]
				}
			}).concat(backendApi.middleware, githubApi.middleware, gitlabApi.middleware);
		}
	});

	// persistStore(store);
	return { store, reducer };
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
		refetchOnFocus: true,
		refetchOnReconnect: true,
		endpoints: (_) => {
			return {};
		}
	});
}

export function createGitLabApi(butlerMod: ReturnType<typeof butlerModule>) {
	return buildCreateApi(
		coreModule(),
		butlerMod
	)({
		reducerPath: 'gitlab',
		tagTypes: Object.values(ReduxTag),
		baseQuery: tauriBaseQuery,
		refetchOnFocus: true,
		refetchOnReconnect: true,
		endpoints: (_) => {
			return {};
		}
	});
}
