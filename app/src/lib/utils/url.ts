import { showToast } from '$lib/notifications/toasts';
import { open } from '@tauri-apps/api/shell';
import { posthog } from 'posthog-js';

export function openExternalUrl(href: string) {
	try {
		open(href);
	} catch (e) {
		if (typeof e == 'string' || e instanceof String) {
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
