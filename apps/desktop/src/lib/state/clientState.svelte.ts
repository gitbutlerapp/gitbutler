import { tauriBaseQuery } from '$lib/state/backendQuery';
import { butlerModule } from '$lib/state/butlerModule';
import { ReduxTag } from '$lib/state/tags';
import { uiStateSlice } from '$lib/state/uiState.svelte';
import { InjectionToken } from '@gitbutler/shared/context';
import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';
import { combineSlices, configureStore, type Reducer } from '@reduxjs/toolkit';
import {
	buildCreateApi,
	coreModule,
	setupListeners,
	type BaseQueryFn,
	type QueryReturnValue,
	type RootState
} from '@reduxjs/toolkit/query';
import { FLUSH, PAUSE, PERSIST, persistReducer, PURGE, REGISTER, REHYDRATE } from 'redux-persist';
import persistStore from 'redux-persist/lib/persistStore';
import storage from 'redux-persist/lib/storage';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { Tauri } from '$lib/backend/tauri';
import type { GitHubClient } from '$lib/forge/github/githubClient';
import type { GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
import type { IrcClient } from '$lib/irc/ircClient.svelte';
import type { ReduxError } from '$lib/state/reduxError';

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

export const CLIENT_STATE = new InjectionToken<ClientState>('ClientState');

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
			getDispatch: () => this.dispatch,
			posthog
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
			gitlabApi: this.gitlabApi
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
}) {
	const { tauri, gitHubClient, gitLabClient, ircClient, backendApi, githubApi, gitlabApi } = params;

	// We can't use the `persistStore` function because it doesn't work
	// with injected reducers. We should inject all reduces so we don't
	// need to know about them in this file.
	const reducer = combineSlices(
		// RTK Query API for the back end.
		backendApi,
		githubApi,
		gitlabApi
	).inject({
		reducerPath: uiStateSlice.reducerPath,
		reducer: persistReducer(
			{
				key: uiStateSlice.reducerPath,
				storage: storage
			},
			uiStateSlice.reducer
		)
	});

	const store = configureStore({
		reducer,
		middleware: (getDefaultMiddleware) => {
			return getDefaultMiddleware({
				thunk: {
					extraArgument: {
						tauri,
						gitHubClient,
						gitLabClient,
						ircClient
					}
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
function createBackendApi(butlerMod: ReturnType<typeof butlerModule>) {
	return buildCreateApi(
		coreModule(),
		butlerMod
	)({
		reducerPath: 'backend',
		tagTypes: Object.values(ReduxTag),
		invalidationBehavior: 'immediately',
		keepUnusedDataFor: 0,
		baseQuery: tauriBaseQuery,
		endpoints: (_) => {
			return {};
		}
	});
}

// Default cache expiration for unused items is 60 seconds. This is too little
// for forge data.
const KEEP_UNUSED_SECONDS = 24 * 60 * 60;

// Fake base query that allows us to use the same error type when the query
// definitions only use `queryFn` instead of `query`.
// eslint-disable-next-line func-style
const fakeBaseQuery: BaseQueryFn = () => {
	return { data: undefined } as QueryReturnValue<never, ReduxError, any>;
};

export function createGitHubApi(butlerMod: ReturnType<typeof butlerModule>) {
	return buildCreateApi(
		coreModule(),
		butlerMod
	)({
		reducerPath: 'github',
		tagTypes: Object.values(ReduxTag),
		invalidationBehavior: 'immediately',
		// TODO: This should only be set for backend api.
		baseQuery: fakeBaseQuery,
		refetchOnFocus: true,
		refetchOnReconnect: true,
		keepUnusedDataFor: KEEP_UNUSED_SECONDS,
		endpoints: (_) => {
			return {};
		}
	});
}

function createGitLabApi(butlerMod: ReturnType<typeof butlerModule>) {
	return buildCreateApi(
		coreModule(),
		butlerMod
	)({
		reducerPath: 'gitlab',
		tagTypes: Object.values(ReduxTag),
		invalidationBehavior: 'immediately',
		// TODO: This should only be set for backend api.
		baseQuery: fakeBaseQuery,
		refetchOnFocus: true,
		refetchOnReconnect: true,
		keepUnusedDataFor: KEEP_UNUSED_SECONDS,
		endpoints: (_) => {
			return {};
		}
	});
}
