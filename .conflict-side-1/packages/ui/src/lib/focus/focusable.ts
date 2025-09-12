import { FOCUS_MANAGER, type FocusableOptions } from '$lib/focus/focusManager';
import { injectOptional } from '@gitbutler/core/context';
import type { Action } from 'svelte/action';

/**
 * Svelte action that registers an element as a focusable area.
 *
 * @example
 * <div use:focusable={{ vertical: true }}>
 */
export function focusable(
	element: HTMLElement,
	options: FocusableOptions = {}
): ReturnType<Action<HTMLElement, FocusableOptions>> {
	const focusManager = injectOptional(FOCUS_MANAGER, undefined);
	if (!focusManager) {
		return {
			destroy() {},
			update() {}
		};
	}

	let currentOptions = options;
	let isRegistered = false;

	function register() {
		if (isRegistered || !focusManager) return;

		try {
			focusManager.register(currentOptions, element);
			isRegistered = true;
		} catch (error) {
			console.warn('Error registering focusable element:', error);
		}
	}

	function unregister() {
		if (!isRegistered || !focusManager) return;

		try {
			focusManager.unregister(element);
			isRegistered = false;
		} catch (error) {
			console.warn('Error unregistering focusable element:', error);
		}
	}

	function handleRegistration(shouldRegister: boolean) {
		if (shouldRegister && !isRegistered) {
			register();
		} else if (!shouldRegister && isRegistered) {
			unregister();
		}
	}

	handleRegistration(!options.disabled);

	return {
		destroy() {
			unregister();
		},

		update(newOptions: FocusableOptions) {
			currentOptions = newOptions;
			handleRegistration(!newOptions.disabled);
			if (isRegistered && focusManager) {
				focusManager.updateElementOptions(element, newOptions);
			}
		}
	};
}
