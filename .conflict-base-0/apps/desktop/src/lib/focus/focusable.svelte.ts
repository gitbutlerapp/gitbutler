import { type Focusable, FOCUS_MANAGER } from '$lib/focus/focusManager.svelte';
import { inject } from '@gitbutler/shared/context';
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
	const focus = inject(FOCUS_MANAGER);

	let { id, parentId = null } = options;
	focus.register(id, parentId, element);

	return {
		destroy() {
			focus.unregister(id);
		},
		update(options) {
			if (id !== options.id) {
				focus.unregister(id);
			}
			id = options.id;
			parentId = options.parentId || null;
			focus.register(id, parentId, element);
		}
	};
};
