import { showToast } from '$lib/notifications/toasts';

const SEPARATOR = '/';

export async function openExternalUrl(href: string) {
	try {
		await invoke<void>('open_url', { url: href });
	} catch (e) {
		if (typeof e === 'string' || e instanceof String) {
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
