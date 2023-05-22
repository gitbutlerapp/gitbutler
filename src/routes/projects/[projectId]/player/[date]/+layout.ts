import { writable } from '@square/svelte-store';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = () => ({
	currentFilepath: writable(''),
	currentSessionId: writable(''),
	currentTimestamp: writable(-1)
});
