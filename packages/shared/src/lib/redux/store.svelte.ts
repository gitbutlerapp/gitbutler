import { branchReviewListingsReducer } from '$lib/branches/branchReviewListingsSlice';
import { branchesReducer } from '$lib/branches/branchesSlice';
import { latestBranchLookupsReducer } from '$lib/branches/latestBranchLookupSlice';
import { chatChannelsReducer } from '$lib/chat/chatChannelsSlice';
import { feedsReducer } from '$lib/feeds/feedsSlice';
import { postsReducer } from '$lib/feeds/postsSlice';
import { organizationsReducer } from '$lib/organizations/organizationsSlice';
import { projectsReducer } from '$lib/organizations/projectsSlice';
import { repositoryIdLookupsReducer } from '$lib/organizations/repositoryIdLookupsSlice';
import { patchEventsReducer } from '$lib/patchEvents/patchEventsSlice';
import { patchSectionsReducer } from '$lib/patches/patchSectionsSlice';
import { patchesReducer } from '$lib/patches/patchesSlice';
import { exampleReducer } from '$lib/redux/example';
import { notificationSettingsReducer } from '$lib/settings/notificationSetttingsSlice';
import { usersByLoginReducer, usersReducer } from '$lib/users/usersSlice';
import { configureStore, createSelector } from '@reduxjs/toolkit';

// Individual interfaces to be used when consuming in other servies.
// By specifying only the interfaces you need, IE:
// `appState: AppPostState & AppExampleState`, it means there is less mocking
// needed when testing.
export interface AppExampleState {
	readonly example: ReturnType<typeof exampleReducer>;
}

export interface AppPostsState {
	readonly posts: ReturnType<typeof postsReducer>;
}

export interface AppFeedsState {
	readonly feeds: ReturnType<typeof feedsReducer>;
}

export interface AppOrganizationsState {
	readonly organizations: ReturnType<typeof organizationsReducer>;
}

export interface AppUsersState {
	readonly users: ReturnType<typeof usersReducer>;
	readonly usersByLogin: ReturnType<typeof usersByLoginReducer>;
}

export interface AppProjectsState {
	readonly projects: ReturnType<typeof projectsReducer>;
}

export interface AppPatchesState {
	readonly patches: ReturnType<typeof patchesReducer>;
}

export interface AppPatchEventsState {
	readonly patchEvents: ReturnType<typeof patchEventsReducer>;
}

export interface AppBranchesState {
	readonly branches: ReturnType<typeof branchesReducer>;
}

export interface AppPatchSectionsState {
	readonly patchSections: ReturnType<typeof patchSectionsReducer>;
}

export interface AppChatChannelsState {
	readonly chatChannels: ReturnType<typeof chatChannelsReducer>;
}

export interface AppRepositoryIdLookupsState {
	readonly repositoryIdLookups: ReturnType<typeof repositoryIdLookupsReducer>;
}

export interface AppLatestBranchLookupsState {
	readonly latestBranchLookups: ReturnType<typeof latestBranchLookupsReducer>;
}

export interface AppBranchReviewListingsState {
	readonly branchReviewListings: ReturnType<typeof branchReviewListingsReducer>;
}

export interface AppNotificationSettingsState {
	readonly notificationSettings: ReturnType<typeof notificationSettingsReducer>;
}

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
		AppNotificationSettingsState
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
			patches: patchesReducer,
			patchEvents: patchEventsReducer,
			branches: branchesReducer,
			patchSections: patchSectionsReducer,
			chatChannels: chatChannelsReducer,
			repositoryIdLookups: repositoryIdLookupsReducer,
			latestBranchLookups: latestBranchLookupsReducer,
			branchReviewListings: branchReviewListingsReducer,
			notificationSettings: notificationSettingsReducer
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

	constructor() {
		$effect(() => {
			const unsubscribe = this._store.subscribe(() => {
				this.rootState = this._store.getState();
			});

			return unsubscribe;
		});
	}
}
