import dayjs from 'dayjs';

export function parseDate(dateStr?: string) {
	return dateStr ? new Date(dateStr) : undefined;
}

export function toHumanReadableTime(d: Date) {
	return d.toLocaleTimeString('en-US', {
		hour: 'numeric',
		minute: 'numeric'
	});
}

export function toHumanReadableDate(d: Date) {
	return d.toLocaleDateString('en-US', {
		dateStyle: 'short',
		hour12: false
	});
}

export function msSinceDaysAgo(days: number) {
	return Math.abs(dayjs().subtract(days, 'day').endOf('day').diff(dayjs(), 'millisecond'));
}
