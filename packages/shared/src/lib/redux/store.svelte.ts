import { branchReviewsReducer } from '$lib/branchReviews/branchReviewsSlice';
import { commitReviewsReducer } from '$lib/branchReviews/commitReviewsSlice';
import { reviewSectionsReducer } from '$lib/branchReviews/reviewSectionsSlice';
import { feedsReducer } from '$lib/feeds/feedsSlice';
import { postsReducer } from '$lib/feeds/postsSlice';
import { organizationsReducer } from '$lib/organizations/organizationsSlice';
import { projectsReducer } from '$lib/organizations/projectsSlice';
import { exampleReducer } from '$lib/redux/example';
import { usersReducer } from '$lib/users/usersSlice';
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
}

export interface AppProjectsState {
	readonly projects: ReturnType<typeof projectsReducer>;
}

export interface AppBranchReviewsState {
	readonly branchReviews: ReturnType<typeof branchReviewsReducer>;
}

export interface AppReviewSectionsState {
	readonly reviewSections: ReturnType<typeof reviewSectionsReducer>;
}

export interface AppCommitReviewsState {
	readonly commitReviews: ReturnType<typeof commitReviewsReducer>;
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
		AppBranchReviewsState,
		AppReviewSectionsState,
		AppCommitReviewsState
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
			projects: projectsReducer,
			branchReviews: branchReviewsReducer,
			reviewSections: reviewSectionsReducer,
			commitReviews: commitReviewsReducer
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
	private readonly selectProjects = createSelector(
		[this.selectSelf],
		(rootState) => rootState.projects
	);
	private readonly selectBranchReviews = createSelector(
		[this.selectSelf],
		(rootState) => rootState.branchReviews
	);
	private readonly selectReviewSections = createSelector(
		[this.selectSelf],
		(rootState) => rootState.reviewSections
	);
	private readonly selectCommitReviews = createSelector(
		[this.selectSelf],
		(rootState) => rootState.commitReviews
	);

	readonly example = $derived(this.selectExample(this.rootState));
	readonly posts = $derived(this.selectPosts(this.rootState));
	readonly feeds = $derived(this.selectFeeds(this.rootState));
	readonly organizations = $derived(this.selectOrganizations(this.rootState));
	readonly users = $derived(this.selectUsers(this.rootState));
	readonly projects = $derived(this.selectProjects(this.rootState));
	readonly branchReviews = $derived(this.selectBranchReviews(this.rootState));
	readonly reviewSections = $derived(this.selectReviewSections(this.rootState));
	readonly commitReviews = $derived(this.selectCommitReviews(this.rootState));

	constructor() {
		$effect(() => {
			const unsubscribe = this._store.subscribe(() => {
				this.rootState = this._store.getState();
			});

			return unsubscribe;
		});
	}
}
