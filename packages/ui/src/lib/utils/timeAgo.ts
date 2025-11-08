import dayjs from 'dayjs';
import localizedFormat from 'dayjs/plugin/localizedFormat';
import relativeTime from 'dayjs/plugin/relativeTime';
import timezone from 'dayjs/plugin/timezone';
import utc from 'dayjs/plugin/utc';
import { writable, type Readable } from 'svelte/store';

dayjs.extend(relativeTime);
dayjs.extend(localizedFormat);
dayjs.extend(utc);
dayjs.extend(timezone);

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

/**
 * Generic helper to create a time-based store that updates at smart intervals
 */
function createTimeBasedStore(
	date: Date | undefined,
	formatFn: (date: Date) => string,
	getUpdateInterval: (seconds: number) => number
): Readable<string> | undefined {
	if (!date) return;
	let timeoutId: number;
	return writable<string>(formatFn(date), (set) => {
		function updateStore() {
			if (!date) return;
			const seconds = Math.round(Math.abs((new Date().getTime() - date.getTime()) / 1000.0));
			const msUntilNextUpdate = Number.isNaN(seconds) ? 1000 : getUpdateInterval(seconds) * 1000;

			set(formatFn(date));

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

export function createTimeAgoStore(
	date: Date | undefined,
	addSuffix: boolean = false
): Readable<string> | undefined {
	return createTimeBasedStore(date, (d) => getTimeAgo(d, addSuffix), getSecondsUntilUpdate);
}

/**
 * Formats a date into an absolute timestamp using browser locale
 * Example: "January 15, 2024 at 3:45 PM" (US) or "15 January 2024 at 15:45" (UK)
 */
export function getAbsoluteTimestamp(input: Date | number | undefined): string {
	if (!input) return '';
	const date = typeof input === 'number' ? new Date(input) : input;

	// Format the date and time using browser locale
	const dateStr = date.toLocaleDateString(undefined, {
		year: 'numeric',
		month: 'long',
		day: 'numeric'
	});
	const timeStr = date.toLocaleTimeString(undefined, {
		hour: '2-digit',
		minute: '2-digit'
	});

	return `${dateStr} at ${timeStr}`;
}

/**
 * Formats a timestamp with dynamic formatting based on age:
 * - < 2 minutes: shows time with seconds using browser locale
 * - < 24 hours: shows time without seconds using browser locale
 * - > 24 hours: shows date and time without seconds using browser locale
 */
export function getTimestamp(input: Date | number): string {
	const date = typeof input === 'number' ? new Date(input) : input;
	const seconds = Math.round(Math.abs((new Date().getTime() - date.getTime()) / 1000.0));

	const twoMinutes = 2 * 60;
	const day = 24 * 60 * 60;

	if (seconds < twoMinutes) {
		// Show time with seconds using browser locale
		return date.toLocaleTimeString(undefined, {
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit'
		});
	} else if (seconds < day) {
		// Show time without seconds using browser locale
		return date.toLocaleTimeString(undefined, {
			hour: '2-digit',
			minute: '2-digit'
		});
	} else {
		// Show date and time without seconds using browser locale
		const dateStr = date.toLocaleDateString(undefined, {
			month: '2-digit',
			day: '2-digit'
		});
		const timeStr = date.toLocaleTimeString(undefined, {
			hour: '2-digit',
			minute: '2-digit'
		});
		return `${dateStr} ${timeStr}`;
	}
}

function getSecondsUntilTimestampUpdate(seconds: number): number {
	const twoMinutes = 2 * 60;
	const day = 24 * 60 * 60;

	if (seconds < twoMinutes) {
		// Update every minute to refresh the seconds display
		return 60;
	} else if (seconds < day) {
		// Update every hour to check if we've crossed the 24h boundary
		return 3600;
	} else {
		// Update every hour in case format needs to change
		return 3600;
	}
}

/**
 * Creates a store that displays a timestamp with dynamic formatting that updates intelligently
 */
export function createTimestampStore(date: Date | undefined): Readable<string> | undefined {
	return createTimeBasedStore(date, getTimestamp, getSecondsUntilTimestampUpdate);
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
