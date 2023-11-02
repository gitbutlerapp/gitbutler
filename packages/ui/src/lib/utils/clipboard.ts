import * as toasts from '$lib/utils/toasts';

export function copyToClipboard(text: string) {
	if (!navigator.clipboard) {
		toasts.error('Clipboard API not available');
	} else {
		navigator.clipboard
			.writeText(text)
			.then(function () {
				toasts.success('Copied to cliboard');
			})
			.catch(function () {
				toasts.error('Failed to copy');
			});
	}
}
