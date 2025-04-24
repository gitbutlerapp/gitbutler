import { Focusable, FocusManager } from '$lib/focus/focusManager.svelte';
import { getContext } from '@gitbutler/shared/context';
import type { Action } from 'svelte/action';

interface FocusableOptions {
	id: Focusable;
	parentId?: Focusable | null;
}

/**
 * Svelte action that registers an element as a focusable area.
 */
// eslint-disable-next-line func-style
export const focusable: Action<HTMLElement, FocusableOptions> = (element, options) => {
	const { id, parentId = null } = options;
	const focus = getContext(FocusManager);

	focus.register(id, parentId, element);
	return {
		destroy() {
			focus.unregister(id);
		},
		update(options) {
			focus.unregister(options.id);
			focus.register(id, parentId, element);
		}
	};
};
