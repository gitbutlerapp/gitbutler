import { dashboardSidebarReducer } from '$lib/dashboard/sidebar.svelte';
import { InjectionToken } from '@gitbutler/core/context';
import { AppDispatch, AppState } from '@gitbutler/shared/redux/store.svelte';
import { configureStore, createSelector, type Store, type Selector } from '@reduxjs/toolkit';

export type WebDashboardSidebarState = {
	readonly dashboardSidebar: ReturnType<typeof dashboardSidebarReducer>;
};

export const WEB_STATE = new InjectionToken<WebState>('WebState');

export class WebState extends AppState implements WebDashboardSidebarState {
	/**
	 * The base store.
	 *
	 * This is a low level API and should not be used directly.
	 * @private
	 */
	readonly _store: Store = configureStore({
		reducer: {
			...this.reducers,
			dashboardSidebar: dashboardSidebarReducer
		}
	});

	readonly appDispatch = new AppDispatch(this._store.dispatch);

	/**
	 * Used to access the store directly. It is recommended to access state via
	 * selectors as they are more efficient.
	 */
	rootState: ReturnType<typeof this._store.getState> = $state<
		ReturnType<typeof this._store.getState>
	>(this._store.getState());

	protected selectSelf: Selector<
		ReturnType<typeof this._store.getState>,
		ReturnType<typeof this._store.getState>
	> = (state: ReturnType<typeof this._store.getState>) => {
		return state;
	};

	private readonly selectDashboardSidebar = createSelector(
		[this.selectSelf],
		(rootState) => rootState.dashboardSidebar
	);

	readonly dashboardSidebar = $derived(this.selectDashboardSidebar(this.rootState));
}
