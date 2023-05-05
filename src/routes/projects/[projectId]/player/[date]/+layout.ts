import { writable } from 'svelte/store';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = () => ({
	currentFilepath: writable(''),
    currentSessionId: writable(''),
});
