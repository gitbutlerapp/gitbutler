import type { RootState } from '$lib/redux/store';

export function selectSelf(state: RootState): RootState {
	return state;
}
