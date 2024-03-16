<script lang="ts">
	import { formatDistanceToNowStrict } from 'date-fns';
	import { writable, type Readable } from 'svelte/store';

	export let date: Date | undefined;
	export let addSuffix = true;
	$: store = createTimeAgoStore(date);

	function createTimeAgoStore(date: Date | undefined): Readable<string> | undefined {
		if (!date) return;
		let timeoutId: number;
		return writable<string>(formatDistanceToNowStrict(date, { addSuffix }), (set) => {
			function updateStore() {
				if (!date) return;
				const seconds = Math.round(Math.abs((new Date().getTime() - date.getTime()) / 1000));
				const msUntilNextUpdate = Number.isNaN(seconds)
					? 1000
					: getSecondsUntilUpdate(seconds) * 1000;
				if (seconds < 60) {
					set('just now');
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
			return 5;
		} else if (seconds < hr) {
			return 15;
		} else if (seconds < day) {
			return 300;
		} else {
			return 3600;
		}
	}
</script>

{#if store}
	{$store}
{/if}
