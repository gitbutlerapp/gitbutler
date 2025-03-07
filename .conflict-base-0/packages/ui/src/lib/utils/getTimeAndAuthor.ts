import { getTimeAgo } from './timeAgo';

export function getTimeAndAuthor(createdAt: Date, name: string | undefined): string {
	const timeAgo = getTimeAgo(createdAt);

	if (name) {
		return `${timeAgo} by ${name}`;
	}

	return timeAgo;
}
