import { branchesReducer } from '@gitbutler/shared/branches/branchesSlice';
import { patchSectionsReducer } from '@gitbutler/shared/branches/patchSectionsSlice';
import { patchesReducer } from '@gitbutler/shared/branches/patchesSlice';
import { feedsReducer } from '@gitbutler/shared/feeds/feedsSlice';
import { postsReducer } from '@gitbutler/shared/feeds/postsSlice';
import { organizationsReducer } from '@gitbutler/shared/organizations/organizationsSlice';
import { projectsReducer } from '@gitbutler/shared/organizations/projectsSlice';
import { exampleReducer } from '@gitbutler/shared/redux/example';
import { AppDispatch, AppState } from '@gitbutler/shared/redux/store.svelte';
import { usersReducer } from '@gitbutler/shared/users/usersSlice';
import { configureStore, createSelector, createSlice } from '@reduxjs/toolkit';

type DesktopOnly = {
	value: number;
};

const desktopOnly = createSlice({
	name: 'desktopOnly',
	initialState: { value: 69 } as DesktopOnly,
	reducers: {
		increment: (state) => {
			state.value += 1;
		},
		decrement: (state) => {
			state.value -= 1;
		}
	}
});

export const { increment: desktopIncrement, decrement: desktopDecrement } = desktopOnly.actions;

export class DesktopDispatch extends AppDispatch {
	constructor(readonly dispatch: typeof DesktopState.prototype._store.dispatch) {
		super(dispatch);
	}
}

interface AppDesktopOnlyState {
	readonly desktopOnly: ReturnType<typeof desktopOnly.reducer>;
}

// There is some minor duplication in terms of what is declared, but we do get
// type errors if you are missing a base reducer in the configureStore call.
// As such, there shouldn't be any concern about the two getting out of sync.
// This is due to limitations in typescript.
export class DesktopState extends AppState implements AppDesktopOnlyState {
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
			branches: branchesReducer,
			patches: patchesReducer,
			patchSections: patchSectionsReducer,
			desktopOnly: desktopOnly.reducer
		}
	});

	readonly appDispatch = new DesktopDispatch(this._store.dispatch);

	/**
	 * Used to access the store directly. It is recommended to access state via
	 * selectors as they are more efficient.
	 */
	rootState = $state<ReturnType<typeof this._store.getState>>(this._store.getState());

	protected selectSelf(state: ReturnType<typeof this._store.getState>) {
		return state;
	}

	private readonly selectDesktopOnly = createSelector(
		[this.selectSelf],
		(rootState) => rootState.desktopOnly
	);
	readonly desktopOnly = $derived(this.selectDesktopOnly(this.rootState));
}
