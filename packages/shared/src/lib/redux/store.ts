import { exampleReducer } from '$lib/redux/example';
import { feedReducer, postReducer, postRepliesReducer } from '$lib/redux/posts/slice';
import { configureStore, createSelector } from '@reduxjs/toolkit';
import { derived, readable, type Readable } from 'svelte/store';

// Individual interfaces to be used when consuming in other servies.
// By specifying only the interfaces you need, IE:
// `appState: AppPostState & AppExampleState`, it means there is less mocking
// needed when testing.
export interface AppExampleState {
	readonly example: Readable<ReturnType<typeof exampleReducer>>;
}

export interface AppPostState {
	readonly post: Readable<ReturnType<typeof postReducer>>;
}

export interface AppPostRepliesState {
	readonly postReplies: Readable<ReturnType<typeof postRepliesReducer>>;
}

export interface AppFeedState {
	readonly feed: Readable<ReturnType<typeof feedReducer>>;
}

export class AppDispatch {
	constructor(readonly dispatch: Dispatch) {}
}

export class AppState implements AppExampleState, AppPostState, AppPostRepliesState, AppFeedState {
	/**
	 * The base store.
	 *
	 * This is a low level API and should not be used directly.
	 * @private
	 */
	readonly _store = configureStore({
		reducer: {
			example: exampleReducer,
			post: postReducer,
			postReplies: postRepliesReducer,
			feed: feedReducer
		}
	});

	readonly appDispatch = new AppDispatch(this._store.dispatch);

	/**
	 * Used to access the store directly. It is recommended to access state via
	 * selectors as they are more efficient.
	 */
	readonly rootState: Readable<RootState> = readable(this._store.getState(), (set) => {
		const unsubscribe = this._store.subscribe(() => {
			set(this._store.getState());
		});
		return unsubscribe;
	});

	private selectSelf(state: RootState) {
		return state;
	}

	private readonly selectExample = createSelector(
		[this.selectSelf],
		(rootState) => rootState.example
	);
	readonly example = derived(this.rootState, this.selectExample);

	private readonly selectPost = createSelector([this.selectSelf], (rootState) => rootState.post);
	readonly post = derived(this.rootState, this.selectPost);

	private readonly selectPostReplies = createSelector(
		[this.selectSelf],
		(rootState) => rootState.postReplies
	);
	readonly postReplies = derived(this.rootState, this.selectPostReplies);

	private readonly selectFeed = createSelector([this.selectSelf], (rootState) => rootState.feed);
	readonly feed = derived(this.rootState, this.selectFeed);
}

export type RootState = ReturnType<typeof AppState.prototype._store.getState>;
export type Dispatch = typeof AppState.prototype._store.dispatch;
