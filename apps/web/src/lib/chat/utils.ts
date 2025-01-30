import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
import type { ChatMessage } from '@gitbutler/shared/chat/types';
import { env } from '$env/dynamic/public';

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
