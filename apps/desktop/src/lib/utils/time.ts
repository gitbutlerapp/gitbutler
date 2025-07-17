import dayjs from 'dayjs';

export function toHumanReadableTime(d: Date) {
	return d.toLocaleTimeString('en-US', {
		hour: 'numeric',
		minute: 'numeric'
	});
}

export function msSinceDaysAgo(days: number) {
	return Math.abs(dayjs().subtract(days, 'day').endOf('day').diff(dayjs(), 'millisecond'));
}
