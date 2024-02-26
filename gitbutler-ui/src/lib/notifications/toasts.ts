import { writable, type Writable } from 'svelte/store';

export type ToastStyle = 'neutral' | 'error' | 'pop' | 'warn';

export interface Toast {
	id?: string;
	message: string;
	title?: string;
	style?: ToastStyle;
}

export const toastStore: Writable<Toast[]> = writable([]);

let idCounter = 0;

export function showToast(toast: Toast) {
	toast.message = toast.message.replace(/^ */gm, '');
	toastStore.update((items) => [
		...items.filter((t) => toast.id == undefined || t.id != toast.id),
		{ id: (idCounter++).toString(), ...toast }
	]);
}

export function dismissToast(messageId: string | undefined) {
	if (!messageId) return;
	toastStore.update((items) => items.filter((m) => m.id != messageId));
}
