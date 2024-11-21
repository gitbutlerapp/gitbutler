import { _store, type AppDispatch, type RootState } from '$lib/redux/store';
import { readable, type Readable } from 'svelte/store';

/**
 * Used to access the store directly. It is recommended to access state via
 * selectors as they are more efficient.
 */
export function useStore(): Readable<RootState> {
	const stateStore = readable(_store.getState(), (set) => {
		const unsubscribe = _store.subscribe(() => {
			set(_store.getState());
		});
		return unsubscribe;
	});

	return stateStore;
}

/**
 * Used to access the dispatch function.
 */
export function useDispatch(): AppDispatch {
	return _store.dispatch;
}
