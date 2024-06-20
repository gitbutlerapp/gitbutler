import { writable, type Writable } from 'svelte/store';
import type { MessageStyle } from '$lib/shared/InfoMessage.svelte';

export interface Toast {
	id?: string;
	message?: string;
	error?: any;
	title?: string;
	style?: MessageStyle;
}

export const toastStore: Writable<Toast[]> = writable([]);

let idCounter = 0;

export function showToast(toast: Toast) {
	toast.message = toast.message?.replace(/^ */gm, '');
	toastStore.update((items) => [
		...items.filter((t) => toast.id === undefined || t.id !== toast.id),
		{ id: (idCounter++).toString(), ...toast }
	]);
}

export function showError(title: string, error: any) {
	// Silence GitHub octokit.js when disconnected
	if (error.status === 500 && error.message === 'Load failed') return;

	const message = error.message || error.toString();
	showToast({ title, error: message, style: 'error' });
}

export function showInfo(title: string, message: string) {
	showToast({ title, message, style: 'neutral' });
}

export function dismissToast(messageId: string | undefined) {
	if (!messageId) return;
	toastStore.update((items) => items.filter((m) => m.id !== messageId));
}
