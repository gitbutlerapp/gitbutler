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
			type: options.type || 'neutral'
		};

		update((chipToasts) => [...chipToasts, chipToast]);

		// Auto-remove after 4 seconds
		setTimeout(() => {
			removeChipToast(id);
		}, 4000);

		return id;
	}

	function removeChipToast(id: string) {
		update((chipToasts) => chipToasts.filter((chipToast) => chipToast.id !== id));
	}

	function clearAll() {
		update(() => []);
	}

	// Convenience methods for different chipToast types
	function neutral(message: string) {
		return addChipToast(message, { type: 'neutral' });
	}

	function success(message: string) {
		return addChipToast(message, { type: 'success' });
	}

	function warning(message: string) {
		return addChipToast(message, { type: 'warning' });
	}

	function error(message: string) {
		return addChipToast(message, { type: 'error' });
	}

	// Keep loading function for compatibility - just an alias for neutral
	function loading(message: string) {
		return neutral(message);
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
