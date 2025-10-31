import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import { writable, type Readable } from 'svelte/store';

dayjs.extend(relativeTime);

function customFormatDistance(date: Date, addSuffix: boolean): string {
	const distance = dayjs(date).fromNow(!addSuffix);
	return distance.replace(
		/\b(seconds?|minutes?|hours?|days?|months?|years?)\b/g,
		(match) => unitShorthandMap[match] ?? ''
	);
}

function getSecondsUntilUpdate(seconds: number) {
	const min = 60;
	const hr = min * 60;
	const day = hr * 24;
	if (seconds < min) {
		return 1;
	} else if (seconds < hr) {
		return 15;
	} else if (seconds < day) {
		return 300;
	} else {
		return 3600;
	}
}

export function getTimeAgo(input: Date | number, addSuffix: boolean = true): string {
	const date = typeof input === 'number' ? new Date(input) : input;

	const seconds = Math.round(Math.abs((new Date().getTime() - date.getTime()) / 1000.0));
	if (seconds < 10) {
		return 'just now';
	} else {
		return customFormatDistance(date, addSuffix);
	}
}

export function createTimeAgoStore(
	date: Date | undefined,
	addSuffix: boolean = false
): Readable<string> | undefined {
	if (!date) return;
	let timeoutId: number;
	return writable<string>(getTimeAgo(date, addSuffix), (set) => {
		function updateStore() {
			if (!date) return;
			const seconds = Math.round(Math.abs((new Date().getTime() - date.getTime()) / 1000.0));
			const msUntilNextUpdate = Number.isNaN(seconds)
				? 1000
				: getSecondsUntilUpdate(seconds) * 1000;

			set(getTimeAgo(date, addSuffix));

			timeoutId = window.setTimeout(() => {
				updateStore();
			}, msUntilNextUpdate);
		}
		updateStore();
		return () => {
			clearTimeout(timeoutId);
		};
	});
}

/**
 * Formats a date into an absolute timestamp in a common, easy-to-read format
 * Example: "January 15, 2024 at 3:45 PM"
 */
export function getAbsoluteTimestamp(input: Date | number): string {
	const date = typeof input === 'number' ? new Date(input) : input;

	// Format the date in a readable format
	return dayjs(date).format('MMMM D, YYYY [at] h:mm A');
}

// SHORTHAND WORDS
const unitShorthandMap: Record<string, string> = {
	second: 'sec',
	seconds: 'sec',
	minute: 'min',
	minutes: 'min',
	hour: 'hour',
	hours: 'hours',
	day: 'day',
	days: 'days',
	month: 'mo',
	months: 'mo',
	year: 'yr',
	years: 'yr'
};
