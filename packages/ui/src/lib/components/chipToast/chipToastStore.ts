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

	return {
		subscribe,
		addChipToast,
		removeChipToast,
		clearAll,
		neutral,
		success,
		warning,
		error
	};
}

export const chipToasts = createChipToastStore();
