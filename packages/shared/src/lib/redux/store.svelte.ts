import { branchReviewListingsReducer } from '$lib/branches/branchReviewListingsSlice';
import { branchesReducer } from '$lib/branches/branchesSlice';
import { latestBranchLookupsReducer } from '$lib/branches/latestBranchLookupSlice';
import { chatChannelsReducer } from '$lib/chat/chatChannelsSlice';
import { feedsReducer } from '$lib/feeds/feedsSlice';
import { postsReducer } from '$lib/feeds/postsSlice';
import { organizationsReducer } from '$lib/organizations/organizationsSlice';
import { projectsReducer } from '$lib/organizations/projectsSlice';
import { recentlyInteractedProjectIdsReducer } from '$lib/organizations/recentlyInteractedProjectIds';
import { recentlyPushedProjectIdsReducer } from '$lib/organizations/recentlyPushedProjectIds';
import { repositoryIdLookupsReducer } from '$lib/organizations/repositoryIdLookupsSlice';
import { patchEventsReducer } from '$lib/patchEvents/patchEventsSlice';
import { patchCommitsReducer } from '$lib/patches/patchCommitsSlice';
import { patchIdablesReducer } from '$lib/patches/patchIdablesSlice';
import { patchSectionsReducer } from '$lib/patches/patchSectionsSlice';
import { exampleReducer } from '$lib/redux/example';
import { notificationSettingsReducer } from '$lib/settings/notificationSetttingsSlice';
import { usersByLoginReducer, usersReducer } from '$lib/users/usersSlice';
import { configureStore, createSelector } from '@reduxjs/toolkit';

// Individual interfaces to be used when consuming in other servies.
// By specifying only the interfaces you need, IE:
// `appState: AppPostState & AppExampleState`, it means there is less mocking
// needed when testing.
export type AppExampleState = {
	readonly example: ReturnType<typeof exampleReducer>;
};

export type AppPostsState = {
	readonly posts: ReturnType<typeof postsReducer>;
};

export type AppFeedsState = {
	readonly feeds: ReturnType<typeof feedsReducer>;
};

export type AppOrganizationsState = {
	readonly organizations: ReturnType<typeof organizationsReducer>;
};

export type AppUsersState = {
	readonly users: ReturnType<typeof usersReducer>;
	readonly usersByLogin: ReturnType<typeof usersByLoginReducer>;
};

export type AppProjectsState = {
	readonly projects: ReturnType<typeof projectsReducer>;
};

export type AppPatchesState = {
	readonly patches: ReturnType<typeof patchCommitsReducer>;
};

export type AppPatchEventsState = {
	readonly patchEvents: ReturnType<typeof patchEventsReducer>;
};

export type AppBranchesState = {
	readonly branches: ReturnType<typeof branchesReducer>;
};

export type AppPatchSectionsState = {
	readonly patchSections: ReturnType<typeof patchSectionsReducer>;
};

export type AppChatChannelsState = {
	readonly chatChannels: ReturnType<typeof chatChannelsReducer>;
};

export type AppRepositoryIdLookupsState = {
	readonly repositoryIdLookups: ReturnType<typeof repositoryIdLookupsReducer>;
};

export type AppLatestBranchLookupsState = {
	readonly latestBranchLookups: ReturnType<typeof latestBranchLookupsReducer>;
};

export type AppBranchReviewListingsState = {
	readonly branchReviewListings: ReturnType<typeof branchReviewListingsReducer>;
};

export type AppNotificationSettingsState = {
	readonly notificationSettings: ReturnType<typeof notificationSettingsReducer>;
};

export type AppPatchIdablesState = {
	readonly patchIdables: ReturnType<typeof patchIdablesReducer>;
};

export type AppRecentlyInteractedProjectIds = {
	readonly recentlyInteractedProjectIds: ReturnType<typeof recentlyInteractedProjectIdsReducer>;
};

export type AppRecentlyPushedProjectIds = {
	readonly recentlyPushedProjectIds: ReturnType<typeof recentlyPushedProjectIdsReducer>;
};

export class AppDispatch {
	constructor(readonly dispatch: typeof AppState.prototype._store.dispatch) {}
}

export class AppState
	implements
		AppExampleState,
		AppPostsState,
		AppFeedsState,
		AppOrganizationsState,
		AppUsersState,
		AppProjectsState,
		AppPatchesState,
		AppPatchEventsState,
		AppBranchesState,
		AppPatchSectionsState,
		AppChatChannelsState,
		AppRepositoryIdLookupsState,
		AppLatestBranchLookupsState,
		AppBranchReviewListingsState,
		AppNotificationSettingsState,
		AppPatchIdablesState,
		AppRecentlyInteractedProjectIds,
		AppRecentlyPushedProjectIds
{
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
			recentlyInteractedProjectIds: recentlyInteractedProjectIdsReducer,
			recentlyPushedProjectIds: recentlyPushedProjectIdsReducer
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
	private readonly selectExample = createSelector(
		[this.selectSelf],
		(rootState) => rootState.examples
	);
	private readonly selectPosts = createSelector([this.selectSelf], (rootState) => rootState.posts);
	private readonly selectFeeds = createSelector([this.selectSelf], (rootState) => rootState.feeds);
	private readonly selectOrganizations = createSelector(
		[this.selectSelf],
		(rootState) => rootState.orgnaizations
	);
	private readonly selectUsers = createSelector([this.selectSelf], (rootState) => rootState.users);
	private readonly selectUsersByLogin = createSelector(
		[this.selectSelf],
		(rootState) => rootState.usersByLogin
	);
	private readonly selectProjects = createSelector(
		[this.selectSelf],
		(rootState) => rootState.projects
	);
	private readonly selectPatches = createSelector(
		[this.selectSelf],
		(rootState) => rootState.patches
	);
	private readonly selectPatchEvents = createSelector(
		[this.selectSelf],
		(rootState) => rootState.patchEvents
	);
	private readonly selectBranches = createSelector(
		[this.selectSelf],
		(rootState) => rootState.branches
	);
	private readonly selectPatchSections = createSelector(
		[this.selectSelf],
		(rootState) => rootState.patchSections
	);
	private readonly selectChatChannels = createSelector(
		[this.selectSelf],
		(rootState) => rootState.chatChannels
	);
	private readonly selectRepositoryIdLookups = createSelector(
		[this.selectSelf],
		(rootState) => rootState.repositoryIdLookups
	);
	private readonly selectLatestBranchLookups = createSelector(
		[this.selectSelf],
		(rootState) => rootState.latestBranchLookups
	);
	private readonly selectBranchReviewListings = createSelector(
		[this.selectSelf],
		(rootState) => rootState.branchReviewListings
	);
	private readonly selectNotificationSettings = createSelector(
		[this.selectSelf],
		(rootState) => rootState.notificationSettings
	);
	private readonly selectPatchIdables = createSelector(
		[this.selectSelf],
		(rootState) => rootState.patchIdables
	);
	private readonly selectRecentlyInteractedProjectIds = createSelector(
		[this.selectSelf],
		(rootState) => rootState.recentlyInteractedProjectIds
	);
	private readonly selectRecentlyPushedProjectIds = createSelector(
		[this.selectSelf],
		(rootState) => rootState.recentlyPushedProjectIds
	);

	readonly example = $derived(this.selectExample(this.rootState));
	readonly posts = $derived(this.selectPosts(this.rootState));
	readonly feeds = $derived(this.selectFeeds(this.rootState));
	readonly organizations = $derived(this.selectOrganizations(this.rootState));
	readonly users = $derived(this.selectUsers(this.rootState));
	readonly usersByLogin = $derived(this.selectUsersByLogin(this.rootState));
	readonly projects = $derived(this.selectProjects(this.rootState));
	readonly patches = $derived(this.selectPatches(this.rootState));
	readonly patchEvents = $derived(this.selectPatchEvents(this.rootState));
	readonly branches = $derived(this.selectBranches(this.rootState));
	readonly patchSections = $derived(this.selectPatchSections(this.rootState));
	readonly chatChannels = $derived(this.selectChatChannels(this.rootState));
	readonly repositoryIdLookups = $derived(this.selectRepositoryIdLookups(this.rootState));
	readonly latestBranchLookups = $derived(this.selectLatestBranchLookups(this.rootState));
	readonly branchReviewListings = $derived(this.selectBranchReviewListings(this.rootState));
	readonly notificationSettings = $derived(this.selectNotificationSettings(this.rootState));
	readonly patchIdables = $derived(this.selectPatchIdables(this.rootState));
	readonly recentlyInteractedProjectIds = $derived(
		this.selectRecentlyInteractedProjectIds(this.rootState)
	);
	readonly recentlyPushedProjectIds = $derived(this.selectRecentlyPushedProjectIds(this.rootState));

	constructor() {
		$effect(() => {
			const unsubscribe = this._store.subscribe(() => {
				this.rootState = this._store.getState();
			});

			return unsubscribe;
		});
	}
}
