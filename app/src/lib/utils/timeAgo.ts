import { formatDistanceToNowStrict } from 'date-fns';
import { writable, type Readable } from 'svelte/store';

export function createTimeAgoStore(
	date: Date | undefined,
	addSuffix: boolean = false
): Readable<string> | undefined {
	if (!date) return;
	let timeoutId: number;
	return writable<string>(formatDistanceToNowStrict(date, { addSuffix }), (set) => {
		function updateStore() {
			if (!date) return;
			const seconds = Math.round(Math.abs((new Date().getTime() - date.getTime()) / 1000.0));
			const msUntilNextUpdate = Number.isNaN(seconds)
				? 1000
				: getSecondsUntilUpdate(seconds) * 1000;
			if (seconds < 10) {
				set('just now');
			} else if (seconds < 60) {
				set(`< 1 min ${addSuffix ? ' ago' : ''}`);
			} else {
				set(customFormatDistance(date, addSuffix));
			}
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

// SHORTHAND WORDS
const unitShorthandMap: Record<string, string> = {
	second: 'sec',
	seconds: 'sec',
	minute: 'min',
	minutes: 'min',
	hour: 'hr',
	hours: 'hr',
	day: 'day',
	days: 'days',
	month: 'mo',
	months: 'mo',
	year: 'yr',
	years: 'yr'
};

function customFormatDistance(date: Date, addSuffix: boolean): string {
	const distance = formatDistanceToNowStrict(date, { addSuffix });
	return distance.replace(
		/\b(seconds?|minutes?|hours?|days?|months?|years?)\b/g,
		(match) => unitShorthandMap[match]
	);
}
