import { tauriBaseQuery } from './backendQuery';
import { butlerModule } from './butlerModule';
import { ReduxTag } from './tags';
import { configureStore } from '@reduxjs/toolkit';
import { buildCreateApi, coreModule } from '@reduxjs/toolkit/query';
import type { Tauri } from '$lib/backend/tauri';
import type { HookContext } from './customHooks.svelte';

/**
 * A redux store with dependency injection through middleware.
 */
export class ClientState {
	private store: ReturnType<typeof createStore>;
	private dispatch: typeof this.store.dispatch;

	// $state requires field declaration, but we have to assign the initial
	// value in the constructor such that we can inject dependencies. The
	// incorrect casting `as` seems difficult to avoid.
	private rootState = $state({} as ReturnType<typeof this.store.getState>);

	/** rtk-query api for communicating with the back end. */
	readonly backendApi: ReturnType<typeof createApi>;

	constructor(readonly tauri: Tauri) {
		this.backendApi = createApi({
			// Reactive loop without nested function.
			// TODO: Can it be done without nesting?
			getState: () => () => this.rootState,
			getDispatch: () => this.dispatch
		});
		this.store = createStore(tauri, this.backendApi);
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
function createStore(tauri: Tauri, backend: ReturnType<typeof createApi>) {
	return configureStore({
		reducer: { backend: backend.reducer },
		middleware: (getDefaultMiddleware) => {
			return getDefaultMiddleware({
				thunk: { extraArgument: { tauri } }
			}).concat(backend.middleware);
		}
	});
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
export function createApi(ctx: HookContext) {
	return buildCreateApi(
		coreModule(),
		butlerModule(ctx)
	)({
		reducerPath: 'backend',
		tagTypes: Object.values(ReduxTag),
		baseQuery: tauriBaseQuery,
		endpoints: (_) => {
			return {};
		}
	});
}
