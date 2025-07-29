import { writable } from 'svelte/store';
import type { ChipToastData, ChipToastOptions } from '$components/chipToast/chipToastTypes';

let toastId = 0;

function generateId(): string {
	return `toast-${++toastId}`;
}

function createChipToastStore() {
	const { subscribe, update } = writable<ChipToastData[]>([]);

	function addChipToast(message: string, options: ChipToastOptions = {}): string {
		const id = generateId();
		const chipToast: ChipToastData = {
			id,
			message,
			type: options.type || 'neutral',
			customButton: options.customButton,
			showDismiss: options.showDismiss
		};

		update((chipToasts) => [...chipToasts, chipToast]);

		// Auto-remove after 4 seconds, but only if dismiss button is not shown
		if (!options.showDismiss) {
			setTimeout(() => {
				removeChipToast(id);
			}, 4000);
		}

		return id;
	}

	function removeChipToast(id: string) {
		update((chipToasts) => chipToasts.filter((chipToast) => chipToast.id !== id));
	}

	function clearAll() {
		update(() => []);
	}

	// Convenience methods for different chipToast types
	function neutral(message: string, options: Omit<ChipToastOptions, 'type'> = {}) {
		return addChipToast(message, { type: 'neutral', ...options });
	}

	function success(message: string, options: Omit<ChipToastOptions, 'type'> = {}) {
		return addChipToast(message, { type: 'success', ...options });
	}

	function warning(message: string, options: Omit<ChipToastOptions, 'type'> = {}) {
		return addChipToast(message, { type: 'warning', ...options });
	}

	function error(message: string, options: Omit<ChipToastOptions, 'type'> = {}) {
		return addChipToast(message, { type: 'error', ...options });
	}

	// Keep loading function for compatibility - just an alias for neutral
	function loading(message: string, options: Omit<ChipToastOptions, 'type'> = {}) {
		return neutral(message, options);
	}

	// Simple promise function that handles loading/success/error states
	async function promise<T>(
		promiseToHandle: Promise<T>,
		opts: {
			loading: string;
			success: string;
			error: string;
		} = {
			loading: 'Loading...',
			success: 'Success!',
			error: 'Error!'
		}
	): Promise<T> {
		const loadingToastId = loading(opts.loading);

		try {
			const result = await promiseToHandle;
			removeChipToast(loadingToastId);
			success(opts.success);
			return result;
		} catch (err) {
			removeChipToast(loadingToastId);
			error(opts.error);
			throw err;
		}
	}

	return {
		subscribe,
		addChipToast,
		removeChipToast,
		clearAll,
		neutral,
		success,
		warning,
		error,
		loading,
		promise
	};
}

export const chipToasts = createChipToastStore();
