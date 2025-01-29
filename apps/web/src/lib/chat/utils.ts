import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
import type { ChatMessage } from '@gitbutler/shared/chat/types';
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

export function messageTimeStamp(message: ChatMessage): string {
	const creationDate = new Date(message.createdAt);
	const hoursAgo = Math.round((Date.now() - creationDate.getTime()) / 1000 / 60 / 60);

	if (hoursAgo < 24) {
		return (
			'Today at ' +
			creationDate.toLocaleTimeString('en-US', {
				hour: 'numeric',
				minute: 'numeric'
			})
		);
	}

	return getTimeAgo(creationDate);
}
