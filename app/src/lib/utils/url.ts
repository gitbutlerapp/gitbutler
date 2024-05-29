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

// URL examples
// GitHub
// git@github.com:gitbutlerapp/gitbutler.git
// https://github.com/gitbutlerapp/gitbutler.git
// GitLab
// git@gitlab.com:vmeet1/vmeet-native.git
// https://gitlab.com/vmeet1/vmeet-native.git
// Gitea
// git@gitea.com:Caleb-T-Owens/bestrepo.git
// https://gitea.com/Caleb-T-Owens/bestrepo.git
// BitBucket
// git@bitbucket.org:calebowens1/bestrepo.git
// https://calebowens1-admin@bitbucket.org/calebowens1/bestrepo.git
// Funky ssh url
// ssh://git@192.168.1.1:22/user/repo.git
// TODO: Consider whether we want to validate input urls: (git@|http:\/\/|https:\/\/|ssh:\/\/git@).*(\/|:).*\/.*\.git

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

export function getOwnerAndRepoFromRemoteUrl(url: string): [string, string] {
	const [owner, repo] = url.replace('.git', '').split('/').slice(-2);
	return [owner, repo];
}
