import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
import { env } from '$env/dynamic/public';

export function getActionCableEndpoint(token: string): string {
	const domain = env.PUBLIC_APP_HOST.replace('http', 'ws');
	const url = new URL('cable', domain);

	const urlSearchParams = new URLSearchParams();
	urlSearchParams.append('token', token);
	url.search = urlSearchParams.toString();

	return url.toString();
}

export type TimestampedEvent = {
	createdAt: string;
	updatedAt: string;
};

export function eventTimeStamp(event: TimestampedEvent): string {
	const creationDate = new Date(event.createdAt);
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
