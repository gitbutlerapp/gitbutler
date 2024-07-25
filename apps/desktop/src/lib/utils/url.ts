import { showToast } from '$lib/notifications/toasts';
import { open } from '@tauri-apps/api/shell';
import GitUrlParse from 'git-url-parse';
import { posthog } from 'posthog-js';

export async function openExternalUrl(href: string) {
	try {
		await open(href);
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
	const gitRemote = GitUrlParse(url);
	const ipv4Regex = new RegExp(/^([0-9]+(\.|$)){4}/);
	const protocol = ipv4Regex.test(gitRemote.resource) ? 'http' : 'https';

	return `${protocol}://${gitRemote.resource}/${gitRemote.owner}/${gitRemote.name}`;
}

export function remoteUrlIsHttp(url: string): boolean {
	const httpProtocols = ['http', 'https'];
	const gitRemote = GitUrlParse(url);

	return httpProtocols.includes(gitRemote.protocol);
}
