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
				set(`< 1 minute ${addSuffix ? ' ago' : ''}`);
			} else {
				set(formatDistanceToNowStrict(date, { addSuffix }));
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
