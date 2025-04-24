import * as toasts from '@gitbutler/ui/toasts';

export function copyToClipboard(text: string) {
	if (!navigator.clipboard) {
		toasts.error('Clipboard API not available');
	} else {
		navigator.clipboard
			.writeText(text)
			.then(function () {
				toasts.success('Copied to clipboard');
			})
			.catch(function (err) {
				toasts.error('Failed to copy');
				console.error('Failed to copy:', err);
			});
	}
}
