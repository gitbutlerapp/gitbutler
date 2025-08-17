import { dashboardSidebarReducer } from '$lib/dashboard/sidebar.svelte';
import { InjectionToken } from '@gitbutler/shared/context';
import { AppDispatch, AppState } from '@gitbutler/shared/redux/store.svelte';
import { configureStore, createSelector } from '@reduxjs/toolkit';

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
	readonly _store = configureStore({
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
	rootState = $state<ReturnType<typeof this._store.getState>>(this._store.getState());

	protected selectSelf(state: ReturnType<typeof this._store.getState>) {
		return state;
	}

	private readonly selectDashboardSidebar = createSelector(
		[this.selectSelf],
		(rootState) => rootState.dashboardSidebar
	);

	readonly dashboardSidebar = $derived(this.selectDashboardSidebar(this.rootState));
}
