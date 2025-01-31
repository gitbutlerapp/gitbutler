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

function isSameDay(date1: Date, date2: Date): boolean {
	return (
		date1.getFullYear() === date2.getFullYear() &&
		date1.getMonth() === date2.getMonth() &&
		date1.getDate() === date2.getDate()
	);
}

export function eventTimeStamp(event: TimestampedEvent): string {
	const creationDate = new Date(event.createdAt);

	const createdToday = isSameDay(creationDate, new Date());

	if (createdToday) {
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
