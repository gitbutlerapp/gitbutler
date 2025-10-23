/**
 * Safely parse a date from various formats (ISO string, Unix timestamp in seconds or milliseconds)
 *
 * @param dateValue - Date as string, number, or Date object
 * @returns Valid Date object or null if parsing fails
 */
export function parseDate(dateValue: string | number | Date | null | undefined): Date | null {
	if (!dateValue) return null;

	// If already a Date object, return it
	if (dateValue instanceof Date) {
		return isNaN(dateValue.getTime()) ? null : dateValue;
	}

	// If it's a number (Unix timestamp)
	if (typeof dateValue === 'number') {
		// Unix timestamps in seconds (10 digits) vs milliseconds (13 digits)
		const timestamp = dateValue < 10000000000 ? dateValue * 1000 : dateValue;
		const date = new Date(timestamp);
		return isNaN(date.getTime()) ? null : date;
	}

	// If it's a string
	if (typeof dateValue === 'string') {
		// Try parsing as Unix timestamp if it's a numeric string
		const numericValue = Number(dateValue);
		if (!isNaN(numericValue) && dateValue.match(/^\d+$/)) {
			const timestamp = numericValue < 10000000000 ? numericValue * 1000 : numericValue;
			const date = new Date(timestamp);
			if (!isNaN(date.getTime())) return date;
		}

		// Try parsing as ISO string or other date format
		const date = new Date(dateValue);
		if (isNaN(date.getTime())) {
			// Assume this format "2025-10-14 18:23:18 +0000"
			const [datePart, timePart, _tzPart] = dateValue.split(' ');
			const [year, month, day] = datePart.split('-').map(Number);
			const [hour, minute, second] = timePart.split(':').map(Number);

			const date2 = new Date(Date.UTC(year, month - 1, day, hour, minute, second));
			return isNaN(date2.getTime()) ? null : date2;
		} else {
			return date;
		}
	}

	return null;
}

/**
 * Formats a date string into a relative time string (e.g., "5 minutes ago", "2 days ago").
 *
 * @param dateString - ISO date string to format
 * @returns Formatted relative time string
 */
export function getRelativeTime(dateString: string): string {
	const date = parseDate(dateString);
	if (!date) return 'Unknown';

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

export function getTimeSince(timestamp: string | number | undefined) {
	if (!timestamp) return 'Unknown';

	const date = parseDate(timestamp);
	if (!date) return 'Unknown';

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
