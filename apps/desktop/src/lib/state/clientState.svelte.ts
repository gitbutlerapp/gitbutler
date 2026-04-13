import { createBackendApi, type BackendApi } from "$lib/state/backendApi";
import { butlerModule } from "$lib/state/butlerModule";
import { messageQueueAdapter, messageQueueSlice } from "$lib/state/messageQueueSlice";
import { ReduxTag } from "$lib/state/tags";
import { uiStateSlice } from "$lib/state/uiState.svelte";
import { InjectionToken } from "@gitbutler/core/context";
import { mergeUnlisten } from "@gitbutler/ui/utils/mergeUnlisten";
import { combineSlices, configureStore, type Slice } from "@reduxjs/toolkit";
import {
	buildCreateApi,
	coreModule,
	setupListeners,
	type BaseQueryFn,
	type QueryReturnValue,
	type RootState,
} from "@reduxjs/toolkit/query";
import { FLUSH, PAUSE, PERSIST, persistReducer, PURGE, REGISTER, REHYDRATE } from "redux-persist";
import persistStore from "redux-persist/lib/persistStore";
import storage from "redux-persist/lib/storage";
import type { IBackend } from "$lib/backend";
// Forge client types are opaque here to avoid circular imports.
// Concrete types are provided at construction time via bootstrap/deps.ts.
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
type GitHubClient = {};
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
type GitLabClient = {};
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
type GiteaClient = {};
import type { ReduxError } from "$lib/error/reduxError";
import type { PostHogWrapper } from "$lib/telemetry/posthog";

/**
 * Backend API object that enables the declaration and usage of endpoints
 * colocated with the feature they support.
 */
export type { BackendApi } from "$lib/state/backendApi";

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
 * Gitea API object that enables the declaration and usage of endpoints
 * colocated with the feature they support.
 */
export type GiteaApi = ReturnType<typeof createGiteaApi>;

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

	// $state requires field declaration, but we have to assign the initial
	// value in the constructor such that we can inject dependencies. The
	// incorrect casting `as` seems difficult to avoid.
	rootState = $state.raw<AppState | undefined>(undefined);
	readonly uiState = $derived(this.rootState?.uiState);

	readonly messageQueue = $derived(
		this.rootState?.messageQueue ?? messageQueueAdapter.getInitialState(),
	);

	/** rtk-query api for communicating with the back end. */
	readonly backendApi: BackendApi;

	/** rtk-query api for communicating with GitHub. */
	readonly githubApi: GitHubApi;

	/** rtk-query api for communicating with GitLab. */
	readonly gitlabApi: GitLabApi;

	/** rtk-query api for communicating with Gitea. */
	readonly giteaApi: GiteaApi;

	constructor(
		backend: IBackend,
		gitHubClient: GitHubClient,
		gitLabClient: GitLabClient,
		giteaClient: GiteaClient,
		posthog: PostHogWrapper,
	) {
		// Cast required: store state has non-RTKQ slices (uiState, messageQueue)
		// that don't satisfy RootState's CombinedState index signature.
		const ctx = {
			getState: () => this.rootState as unknown as RootState<any, any, any>,
			getDispatch: () => this.dispatch,
			posthog,
		};
		this.backendApi = createBackendApi(ctx);

		const butlerMod = butlerModule(ctx);
		this.githubApi = createGitHubApi(butlerMod);
		this.gitlabApi = createGitLabApi(butlerMod);
		this.giteaApi = createGiteaApi(butlerMod);

		const { store, reducer } = createStore({
			backend,
			gitHubClient,
			gitLabClient,
			giteaClient,
			backendApi: this.backendApi,
			githubApi: this.githubApi,
			gitlabApi: this.gitlabApi,
			giteaApi: this.giteaApi,
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

	initPersist() {
		persistStore(this.store);
	}

	/**
	 * Inject a persisted slice into the store and return a reactive getter
	 * for the slice state. Consumers should use this in an `$effect` to
	 * keep a local `$state.raw` field in sync.
	 */
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

/**
 * We need this function in order to declare the store type in `DesktopState`
 * and then assign the value in the constructor.
 */
function createStore(params: {
	backend: IBackend;
	gitHubClient: GitHubClient;
	gitLabClient: GitLabClient;
	giteaClient: GiteaClient;
	backendApi: BackendApi;
	githubApi: GitHubApi;
	gitlabApi: GitLabApi;
	giteaApi: GiteaApi;
}) {
	const { backend, gitHubClient, gitLabClient, giteaClient, backendApi, githubApi, gitlabApi, giteaApi } = params;

	// We can't use the `persistStore` function because it doesn't work
	// with injected reducers. We should inject all reduces so we don't
	// need to know about them in this file.
	const reducer = combineSlices(
		// RTK Query API for the back end.
		backendApi,
		githubApi,
		gitlabApi,
		giteaApi,
	)
		.inject({
			reducerPath: uiStateSlice.reducerPath,
			reducer: persistReducer(
				{
					key: uiStateSlice.reducerPath,
					storage: storage,
				},
				uiStateSlice.reducer,
			),
		})
		.inject({
			reducerPath: messageQueueSlice.reducerPath,
			reducer: persistReducer(
				{ key: messageQueueSlice.reducerPath, storage },
				messageQueueSlice.reducer,
			),
		});

	const store = configureStore({
		reducer,
		middleware: (getDefaultMiddleware) => {
			return getDefaultMiddleware({
				thunk: {
					extraArgument: {
						backend,
						gitHubClient,
						gitLabClient,
						giteaClient,
					},
				},
				serializableCheck: {
					ignoredActions: [FLUSH, REHYDRATE, PAUSE, PERSIST, PURGE, REGISTER],
					// skip the serializable check for rtk-query cache (contains only serializable data)
					ignoredPaths: ["backend", "github", "gitlab", "gitea"],
				},
			}).concat(backendApi.middleware, githubApi.middleware, gitlabApi.middleware, giteaApi.middleware);
		},
	});

	return { store, reducer };
}

// Default cache expiration for unused items is 60 seconds. This is too little
// for forge data, so we keep forge data cached for 24 hours.
const FORGE_CACHE_TTL_SECONDS = 24 * 60 * 60; // 24 hours

// Fake base query that allows us to use the same error type when the query
// definitions only use `queryFn` instead of `query`. Intentionally typed as
// bare BaseQueryFn for compatibility with the butlerModule type augmentation.
// eslint-disable-next-line func-style
const fakeBaseQuery: BaseQueryFn = () => {
	return { data: undefined } as QueryReturnValue<never, ReduxError, any>;
};

// Common API configuration for forge APIs
const FORGE_API_CONFIG = {
	tagTypes: Object.values(ReduxTag),
	invalidationBehavior: "immediately" as const,
	baseQuery: fakeBaseQuery,
	refetchOnFocus: true,
	refetchOnReconnect: true,
	keepUnusedDataFor: FORGE_CACHE_TTL_SECONDS,
	endpoints: () => ({}),
};

export function createGitHubApi(butlerMod: ReturnType<typeof butlerModule>) {
	return buildCreateApi(
		coreModule(),
		butlerMod,
	)({
		reducerPath: "github",
		// Using fake base query for forge APIs (GitHub/GitLab) since they use queryFn
		...FORGE_API_CONFIG,
	});
}

export function createGitLabApi(butlerMod: ReturnType<typeof butlerModule>) {
	return buildCreateApi(
		coreModule(),
		butlerMod,
	)({
		reducerPath: "gitlab",
		// Using fake base query for forge APIs (GitHub/GitLab) since they use queryFn
		...FORGE_API_CONFIG,
	});
}

export function createGiteaApi(butlerMod: ReturnType<typeof butlerModule>) {
	return buildCreateApi(
		coreModule(),
		butlerMod,
	)({
		reducerPath: "gitea",
		// Using fake base query for forge APIs (GitHub/GitLab) since they use queryFn
		...FORGE_API_CONFIG,
	});
}
