import { chipToasts } from '@gitbutler/ui';

export function copyToClipboard(text: string) {
	if (!navigator.clipboard) {
		chipToasts.error('Clipboard API not available');
	} else {
		navigator.clipboard
			.writeText(text)
			.then(function () {
				chipToasts.success('Copied to clipboard');
			})
			.catch(function (err) {
				chipToasts.error('Failed to copy');
				console.error('Failed to copy:', err);
			});
	}
}
