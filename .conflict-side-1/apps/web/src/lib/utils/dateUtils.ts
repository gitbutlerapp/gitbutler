/**
 * Formats a date string into a relative time string (e.g., "5 minutes ago", "2 days ago").
 *
 * @param dateString - ISO date string to format
 * @returns Formatted relative time string
 */
export function getRelativeTime(dateString: string): string {
	const date = new Date(dateString);
	const utcDate = new Date(date.getTime() + date.getTimezoneOffset() * 60000);
	const now = new Date();
	const diffInSeconds = Math.floor((now.getTime() - utcDate.getTime()) / 1000);

	if (diffInSeconds < 60) {
		return `${diffInSeconds} seconds ago`;
	}
	if (diffInSeconds < 3600) {
		return `${Math.floor(diffInSeconds / 60)} minutes ago`;
	}
	if (diffInSeconds < 86400) {
		return `${Math.floor(diffInSeconds / 3600)} hours ago`;
	}
	if (diffInSeconds < 2592000) {
		return `${Math.floor(diffInSeconds / 86400)} days ago`;
	}
	if (diffInSeconds < 31536000) {
		return `${Math.floor(diffInSeconds / 2592000)} months ago`;
	}
	return `${Math.floor(diffInSeconds / 31536000)} years ago`;
}

export function getTimeSince(timestamp: string | undefined) {
	if (!timestamp) return 'Unknown';

	const date = new Date(timestamp);
	const now = new Date();
	const diffTime = Math.abs(now.getTime() - date.getTime());
	const diffDays = Math.floor(diffTime / (1000 * 60 * 60 * 24));

	if (diffDays === 0) {
		return 'Today';
	} else if (diffDays === 1) {
		return 'Yesterday';
	} else if (diffDays < 7) {
		return `${diffDays} days ago`;
	} else if (diffDays < 30) {
		return `${Math.floor(diffDays / 7)} weeks ago`;
	} else if (diffDays < 365) {
		return `${Math.floor(diffDays / 30)} months ago`;
	} else {
		return `${Math.floor(diffDays / 365)} years ago`;
	}
}
