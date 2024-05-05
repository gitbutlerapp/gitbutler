import { writable, type Writable } from 'svelte/store';
import type { MessageStyle } from '$lib/components/InfoMessage.svelte';

export interface Toast {
	id?: string;
	message?: string;
	errorMessage?: any;
	title?: string;
	style?: MessageStyle;
}

export const toastStore: Writable<Toast[]> = writable([]);

let idCounter = 0;

export function showToast(toast: Toast) {
	toast.message = toast.message?.replace(/^ */gm, '');
	toastStore.update((items) => [
		...items.filter((t) => toast.id == undefined || t.id != toast.id),
		{ id: (idCounter++).toString(), ...toast }
	]);
}

export function showError(title: string, err: any) {
	const errorMessage = err.message ? err.message : err;
	showToast({ title, errorMessage: errorMessage, style: 'error' });
}

export function dismissToast(messageId: string | undefined) {
	if (!messageId) return;
	toastStore.update((items) => items.filter((m) => m.id != messageId));
}
