import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
import type { UserMaybe } from '@gitbutler/shared/users/types';

const UNKNOWN_AUTHOR = 'Unknown author';

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

export function getMultipleContributorNames(contributors: UserMaybe[]): string {
	if (contributors.length === 0) {
		return UNKNOWN_AUTHOR;
	}

	return contributors
		.map((contributor) => {
			if (contributor.user) {
				const user = contributor.user;
				return user.login ?? user.name ?? user.email ?? UNKNOWN_AUTHOR;
			} else {
				return contributor.email;
			}
		})
		.join(', ');
}
