import { parseError } from '$lib/error/parser';
import posthog from 'posthog-js';
import { writable, type Writable } from 'svelte/store';
import type { MessageStyle } from '$components/InfoMessage.svelte';

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
	if (toast.error) {
		// TODO: Make toast a service, so we can inject posthog.
		posthog.capture('toast:show_error', {
			error_title: toast.title,
			error_message: String(toast.error)
		});
	}
	toast.message = toast.message?.replace(/^ */gm, '');
	toastStore.update((items) => [
		...items.filter((t) => toast.id === undefined || t.id !== toast.id),
		{ id: (idCounter++).toString(), ...toast }
	]);
}

export function showError(title: string, error: unknown) {
	const { message, parsedError } = parseError(error);
	if (parsedError) {
		showToast({ title, message, error: parsedError, style: 'error' });
	}
}

export function showInfo(title: string, message: string) {
	showToast({ title, message, style: 'neutral' });
}

export function dismissToast(messageId: string | undefined) {
	if (!messageId) return;
	toastStore.update((items) => items.filter((m) => m.id !== messageId));
}
