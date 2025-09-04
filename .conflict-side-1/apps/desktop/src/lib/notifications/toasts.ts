import { getTitleFromCommonErrorMessage, isBundlingError, parseError } from '$lib/error/parser';
import posthog from 'posthog-js';
import { writable, type Writable } from 'svelte/store';
import type { MessageStyle } from '$components/InfoMessage.svelte';

type ExtraAction = {
	label: string;
	onClick: (dismiss: () => void) => void;
};

export interface Toast {
	id?: string;
	message?: string;
	error?: any;
	title?: string;
	style?: MessageStyle;
	extraAction?: ExtraAction;
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

export function showError(title: string, error: unknown, extraAction?: ExtraAction) {
	const { name, message, description, ignored } = parseError(error);
	if (isBundlingError(message)) {
		console.warn(
			'You are likely experiencing a dev mode bundling error, ' +
				'try disabling the chache from the network tab and ' +
				'reload the page.'
		);
		return;
	}
	if (!ignored) {
		const commonErrorTitle = getTitleFromCommonErrorMessage(message);
		showToast({
			title: name || commonErrorTitle || title,
			message: description,
			error: message,
			style: 'error',
			extraAction
		});
	}
}

export function showInfo(title: string, message: string, extraAction?: ExtraAction) {
	showToast({ title, message, style: 'neutral', extraAction });
}

export function showWarning(title: string, message: string, extraAction?: ExtraAction) {
	showToast({ title, message, style: 'warning', extraAction });
}

export function dismissToast(messageId: string | undefined) {
	if (!messageId) return;
	toastStore.update((items) => items.filter((m) => m.id !== messageId));
}
