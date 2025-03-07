import { dashboardSidebarReducer } from '$lib/dashboard/sidebar.svelte';
import { branchReviewListingsReducer } from '@gitbutler/shared/branches/branchReviewListingsSlice';
import { branchesReducer } from '@gitbutler/shared/branches/branchesSlice';
import { latestBranchLookupsReducer } from '@gitbutler/shared/branches/latestBranchLookupSlice';
import { chatChannelsReducer } from '@gitbutler/shared/chat/chatChannelsSlice';
import { feedsReducer } from '@gitbutler/shared/feeds/feedsSlice';
import { postsReducer } from '@gitbutler/shared/feeds/postsSlice';
import { organizationsReducer } from '@gitbutler/shared/organizations/organizationsSlice';
import { projectsReducer } from '@gitbutler/shared/organizations/projectsSlice';
import { repositoryIdLookupsReducer } from '@gitbutler/shared/organizations/repositoryIdLookupsSlice';
import { patchEventsReducer } from '@gitbutler/shared/patchEvents/patchEventsSlice';
import { patchCommitsReducer } from '@gitbutler/shared/patches/patchCommitsSlice';
import { patchIdablesReducer } from '@gitbutler/shared/patches/patchIdablesSlice';
import { patchSectionsReducer } from '@gitbutler/shared/patches/patchSectionsSlice';
import { exampleReducer } from '@gitbutler/shared/redux/example';
import { AppDispatch, AppState } from '@gitbutler/shared/redux/store.svelte';
import { notificationSettingsReducer } from '@gitbutler/shared/settings/notificationSetttingsSlice';
import { usersReducer, usersByLoginReducer } from '@gitbutler/shared/users/usersSlice';
import { configureStore, createSelector } from '@reduxjs/toolkit';

export type WebDashboardSidebarState = {
	readonly dashboardSidebar: ReturnType<typeof dashboardSidebarReducer>;
};

export class WebState extends AppState implements WebDashboardSidebarState {
	/**
	 * The base store.
	 *
	 * This is a low level API and should not be used directly.
	 * @private
	 */
	readonly _store = configureStore({
		reducer: {
			examples: exampleReducer,
			posts: postsReducer,
			feeds: feedsReducer,
			orgnaizations: organizationsReducer,
			users: usersReducer,
			usersByLogin: usersByLoginReducer,
			projects: projectsReducer,
			patches: patchCommitsReducer,
			patchEvents: patchEventsReducer,
			branches: branchesReducer,
			patchSections: patchSectionsReducer,
			chatChannels: chatChannelsReducer,
			repositoryIdLookups: repositoryIdLookupsReducer,
			latestBranchLookups: latestBranchLookupsReducer,
			branchReviewListings: branchReviewListingsReducer,
			notificationSettings: notificationSettingsReducer,
			patchIdables: patchIdablesReducer,
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
