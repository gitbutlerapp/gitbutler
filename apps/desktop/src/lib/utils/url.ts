import { showToast } from '$lib/notifications/toasts';
import { InjectionToken } from '@gitbutler/core/context';
import type { IBackend } from '$lib/backend';

const SEPARATOR = '/';

export const URL_SERVICE = new InjectionToken<URLService>('URLService');

export default class URLService {
	constructor(private backend: IBackend) {}

	async openExternalUrl(href: string) {
		try {
			await this.backend.openExternalUrl(href);
		} catch (e) {
			if (typeof e === 'string' || e instanceof String) {
				const message = `
                Failed to open link in external browser:

                ${href}
            `;
				showToast({ title: 'External URL error', message, style: 'danger' });
			}

			// Rethrowing for sentry and posthog
			throw e;
		}
	}
}

export interface EditorUriParams {
	schemeId: string;
	path: string[];
	searchParams?: Record<string, string>;
	line?: number;
	column?: number;
}

export function getEditorUri(params: EditorUriParams): string {
	const searchParamsString = new URLSearchParams(params.searchParams).toString();
	// Separator is always a forward slash for editor paths, even on Windows
	const pathString = params.path.join(SEPARATOR);

	let positionSuffix = '';
	if (params.line !== undefined) {
		positionSuffix += `:${params.line}`;
		// Column is only valid if line is present
		if (params.column !== undefined) {
			positionSuffix += `:${params.column}`;
		}
	}

	const searchSuffix = searchParamsString ? `?${searchParamsString}` : '';

	return `${params.schemeId}://file${pathString}${positionSuffix}${searchSuffix}`;
}
