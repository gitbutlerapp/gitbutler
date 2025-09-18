import { persistSwallowGitHubOrgAuthErrors } from '$lib/config/config';
import {
	getTitleFromCommonErrorMessage,
	isBundlingError,
	isGitHubOrgAuthError,
	parseError,
	shouldIgnoreThistError
} from '$lib/error/parser';
import posthog from 'posthog-js';
import { writable, type Writable } from 'svelte/store';
import type { MessageStyle } from '$components/InfoMessage.svelte';

type ExtraAction = {
	label: string;
	testId?: string;
	onClick: (dismiss: () => void) => void;
};

export interface Toast {
	id?: string;
	testId?: string;
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
		posthog.capture('toast:show_error', {
			error_test_id: toast.testId,
			error_title: toast.title,
			error_message: String(toast.error)
		});
	}

	if (toast.style === 'warning') {
		posthog.capture('toast:show_warning', {
			warning_test_id: toast.testId,
			warning_title: toast.title,
			warning_message: toast.message
		});
	}

	toast.message = toast.message?.replace(/^ */gm, '');
	if (!toast.id) {
		toast = { ...toast, id: `${idCounter++}` };
	}
	toastStore.update((items) => [
		...items.filter((t) => toast.id === undefined || t.id !== toast.id),
		toast
	]);
}

export function showError(title: string, error: unknown, extraAction?: ExtraAction, id?: string) {
	const { name, message, description, ignored } = parseError(error);
	if (isBundlingError(message)) {
		console.warn(
			'You are likely experiencing a dev mode bundling error, ' +
				'try disabling the chache from the network tab and ' +
				'reload the page.'
		);
		return;
	}
	const commonErrorTitle = getTitleFromCommonErrorMessage(message);
	const actualTitle = name || commonErrorTitle || title;
	const shouldIgnoreThisSpecificError = shouldIgnoreThistError(actualTitle);

	if (!ignored && !shouldIgnoreThisSpecificError) {
		const offerToIgnore = isGitHubOrgAuthError(actualTitle);
		const actualExtraAction =
			extraAction ??
			(offerToIgnore
				? {
						label: "Don't show this again",
						onClick: () => {
							persistSwallowGitHubOrgAuthErrors(true);
						}
					}
				: undefined);

		showToast({
			id,
			title: actualTitle,
			message: description,
			error: message,
			style: 'error',
			extraAction: actualExtraAction
		});
	}
}

export function showInfo(title: string, message: string, extraAction?: ExtraAction) {
	showToast({ title, message, style: 'info', extraAction });
}

export function showWarning(title: string, message: string, extraAction?: ExtraAction) {
	showToast({ title, message, style: 'warning', extraAction });
}

export function dismissToast(messageId: string | undefined) {
	if (!messageId) return;
	toastStore.update((items) => items.filter((m) => m.id !== messageId));
}
