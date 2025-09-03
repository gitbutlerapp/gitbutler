import { FOCUS_MANAGER, type FocusableOptions, type Payload } from '$lib/focus/focusManager';
import { injectOptional } from '@gitbutler/core/context';
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
export function focusable(
	element: HTMLElement,
	options: FocusableOptions = {}
): ReturnType<Action<HTMLElement, FocusableOptions>> {
	const focusManager = injectOptional(FOCUS_MANAGER, undefined);
	if (!focusManager) return;

	let currentOptions = options;
	let isRegistered = false;

	function register() {
		if (isRegistered || !focusManager) return;

		focusManager.register(currentOptions, element);
		isRegistered = true;
	}

	function unregister() {
		if (!isRegistered || !focusManager) return;

		focusManager.unregister(element);
		isRegistered = false;
	}

	if (!options.disabled) {
		register();
	}

	return {
		destroy() {
			unregister();
		},

		update(newOptions: FocusableOptions<Payload>) {
			const oldId = currentOptions.id;
			const newId = newOptions.id;

			// If the ID changed, we need to unregister and re-register
			if (oldId !== newId) {
				unregister();
				currentOptions = newOptions;
				if (!currentOptions.disabled) {
					register();
				}
			} else {
				if (!currentOptions.disabled && newOptions.disabled) {
					unregister();
				}
				currentOptions = newOptions;

				// Update the existing registration
				focusManager.updateElementOptions(element, newOptions);
			}
		}
	};
}
