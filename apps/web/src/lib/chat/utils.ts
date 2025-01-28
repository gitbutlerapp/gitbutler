import { env } from '$env/dynamic/public';

export type SubscriptionEvent = {
	uuid: string;
	event_type: string;
	data: unknown;
	object: unknown;
	created_at: string;
	updated_at: string;
};

export function isSubscriptionEvent(data: unknown): data is SubscriptionEvent {
	return (
		typeof data === 'object' &&
		data !== null &&
		typeof (data as any).uuid === 'string' &&
		typeof (data as any).event_type === 'string' &&
		typeof (data as any).created_at === 'string' &&
		typeof (data as any).updated_at === 'string'
	);
}

export function getActionCableEndpoint(token: string): string {
	const domain = env.PUBLIC_APP_HOST.replace('http', 'ws');
	const url = new URL('cable', domain);

	const urlSearchParams = new URLSearchParams();
	urlSearchParams.append('token', token);
	url.search = urlSearchParams.toString();

	return url.toString();
}
