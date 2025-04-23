import { VERSION } from './version.js';
import { getUserAgent } from 'universal-user-agent';

export const HOST = process.env.GITBUTLER_API_HOST ?? 'https://app.gitbutler.com';
export const API_BASE_URL = `${HOST}/api/`;
export const USER_AGENT = `gitbutler-mcp/${VERSION} ${getUserAgent()}`;

export type RequestOptions = {
	method?: 'POST' | 'GET' | 'PUT' | 'DELETE';
	headers?: Record<string, string>;
	body?: FormData | string | object;
};

type ParameterValue = string | number | boolean | undefined;

export function getGitbutlerAPIKey(): string | undefined {
	return process.env.GITBUTLER_API_KEY;
}

export function hasGitButlerAPIKey(): boolean {
	return !!getGitbutlerAPIKey();
}

async function parseResponseBody(response: Response): Promise<unknown> {
	const contentType = response.headers.get('Content-Type');
	if (contentType?.includes('application/json')) {
		return await response.json();
	}
	return await response.text();
}

export function interpolatePath(path: string, params: Record<string, string>): string {
	return Object.entries(params).reduce(
		(interpolatedPath, [key, value]) =>
			interpolatedPath.replace(new RegExp(`{${key}}`, 'g'), encodeURIComponent(value)),
		path
	);
}

export function getGitbutlerAPIUrl(
	path: string,
	params: Record<string, ParameterValue> = {}
): string {
	const url = new URL(`.${path}`, API_BASE_URL);
	Object.entries(params).forEach(([key, value]) => {
		if (value === undefined) return;
		url.searchParams.append(key, value.toString());
	});
	return url.toString();
}

export async function gitbutlerAPIRequest(
	url: string,
	options: RequestOptions = {}
): Promise<unknown> {
	const { method = 'GET', headers: optionHeaders, body } = options;

	const headers: Record<string, string> = {
		...optionHeaders,
		'User-Agent': USER_AGENT,
		'Content-Type': 'application/json'
	};

	if (process.env.GITBUTLER_API_KEY) {
		headers['X-Auth-Token'] = process.env.GITBUTLER_API_KEY;
	}

	const response = await fetch(url, {
		method,
		headers,
		body: body ? JSON.stringify(body) : undefined
	});

	const responseBody = await parseResponseBody(response);

	if (!response.ok) {
		throw new Error(
			`Request failed: ${response.status} ${response.statusText} - ${JSON.stringify(responseBody)}`
		);
	}

	return responseBody;
}
