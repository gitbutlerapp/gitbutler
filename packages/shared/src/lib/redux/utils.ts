import { _store, type AppDispatch, type RootState } from '$lib/redux/store';
import { derived, readable, type Readable } from 'svelte/store';

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
 * Used in combination with slice specific selectors. If an argument of the
 * selector depends on a reactive property, IE something defined with
 * `$state` or similar runes, then the `useSelector` call should be wrapped in
 * a `$derived` block.
 */
export function useSelector<T>(selector: (state: RootState) => T): Readable<T> {
	const stateStore = useStore();

	return derived(stateStore, (state) => selector(state));
}

/**
 * Used to access the dispatch function.
 */
export function useDispatch(): AppDispatch {
	return _store.dispatch;
}
