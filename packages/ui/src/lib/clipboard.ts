import * as toasts from '$lib/toasts';

export function copyToClipboard(text: string) {
	if (!navigator.clipboard) {
		toasts.error('Clipboard API not available');
	} else {
		navigator.clipboard
			.writeText(text)
			.then(function () {
				toasts.success('SSH key copied to cliboard');
			})
			.catch(function () {
				toasts.error('Failed to copy SSH key');
			});
	}
}
