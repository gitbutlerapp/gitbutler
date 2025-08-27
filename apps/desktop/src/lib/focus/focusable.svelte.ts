import { FOCUS_MANAGER, type FocusableOptions } from '$lib/focus/focusManager';
import { inject } from '@gitbutler/shared/context';
import type { Action } from 'svelte/action';

/**
 * Svelte action that registers an element as a focusable area.
 *
 * @example
 * <div use:focusable={{
 *   id: 'stack',
 *   parentId: 'workspace',
 *   payload: { stackId: 'abc123', branchName: 'feature' },
 *   onKeydown: (event, context) => handleStackKeys(event, context.payload),
 *   onFocus: (context) => highlightStack(context.payload.stackId)
 * }}>
 */
export function focusable<TPayload extends object>(
	element: HTMLElement,
	options?: FocusableOptions<TPayload>
): ReturnType<Action<HTMLElement, FocusableOptions<TPayload>>> {
	if (!options) {
		options = {};
	}
	const focus = inject(FOCUS_MANAGER);

	let currentOptions = options;
	let isRegistered = false;

	function register() {
		if (isRegistered) return;

		focus.register(currentOptions, element);
		isRegistered = true;
	}

	function unregister() {
		if (!isRegistered) return;

		focus.unregister(currentOptions.id, element);
		isRegistered = false;
	}

	// Initial registration
	if (!options.disabled) {
		register();
	}

	return {
		destroy() {
			unregister();
		},

		update(newOptions: FocusableOptions<TPayload>) {
			// If the ID changed, we need to unregister and re-register
			const oldId = currentOptions.id;
			const newId = newOptions.id;

			if (oldId !== newId) {
				unregister();
				currentOptions = newOptions;
				if (!currentOptions.disabled) {
					register();
				}
			} else {
				// Same ID - we can update in place
				if (!currentOptions.disabled && newOptions.disabled) {
					unregister();
				}
				currentOptions = newOptions;

				// Update the existing registration
				focus.updateElementOptions(element, newOptions);
			}
		}
	};
}
