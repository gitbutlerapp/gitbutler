import { showToast } from '$lib/notifications/toasts';
import { open } from '@tauri-apps/api/shell';
import { posthog } from 'posthog-js';

export function openExternalUrl(href: string) {
	try {
		open(href);
	} catch (e) {
		if (typeof e === 'string' || e instanceof String) {
			// TODO: Remove if/when we've resolved all external URL problems.
			posthog.capture('Link Error', { href, message: e });

			const message = `
                Failed to open link in external browser:

                ${href}
            `;
			showToast({ title: 'External URL error', message, style: 'error' });
		}

		// Rethrowing for sentry and posthog
		throw e;
	}
}

// turn a git remote url into a web url (github, gitlab, bitbucket, etc)
export function convertRemoteToWebUrl(url: string): string {
	if (url.startsWith('http')) {
		return url.replace('.git', '').trim();
	} else if (url.startsWith('ssh')) {
		url = url.replace('ssh://git@', '');
		const [host, ...paths] = url.split('/');
		const path = paths.join('/').replace('.git', '');
		const protocol = /\d+\.\d+\.\d+\.\d+/.test(host) ? 'http' : 'https';
		const [hostname, _port] = host.split(':');
		return `${protocol}://${hostname}/${path}`;
	} else {
		return url.replace(':', '/').replace('git@', 'https://').replace('.git', '').trim();
	}
}
