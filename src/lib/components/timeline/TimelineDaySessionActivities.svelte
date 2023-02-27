<script lang="ts">
	import type { Activity } from '$lib/sessions';
	import {
		IconCircleHalf2,
		IconMapPinFilled,
		IconSquareRoundedFilled,
		IconCircleFilled
	} from '@tabler/icons-svelte';

	export let activities: Activity[];
	export let sessionStart: number;
	export let sessionEnd: number;

	$: sessionDuration = sessionEnd - sessionStart;

	let proportionOfTime = (time: number) => {
		return ((time - sessionStart) / sessionDuration) * 100;
	};
	const toHumanReadableTime = (timestamp: number) => {
		return new Date(timestamp).toLocaleTimeString('en-US', {
			hour: 'numeric',
			minute: 'numeric'
		});
	};
</script>

<div class="relative">
	<hr class="h-px bg-slate-400 border-0 z-0" />
	<div class="absolute inset-0 -mt-1.5">
		{#each activities as activity}
			<div
				class="flex -mx-1.5"
				style="position:relative; left: {proportionOfTime(activity.timestampMs)}%;"
			>
				<div
					class="w-3 h-3 text-slate-700 z-50 absolute inset-0"
					style=""
					title="{activity.type}: {activity.message} at {toHumanReadableTime(activity.timestampMs)}"
				>
					{#if activity.type.startsWith('commit')}
						<IconSquareRoundedFilled class="w-3 h-3 text-sky-500 hover:text-sky-600" />
					{:else if activity.type.startsWith('merge')}
						<IconMapPinFilled class="w-3 h-3 text-green-500 hover:text-green-600" />
					{:else if activity.type.startsWith('rebase')}
						<IconCircleHalf2 class="w-3 h-3 text-orange-500 hover:text-orange-600" />
					{:else if activity.type.startsWith('push')}
						<IconCircleFilled class="w-3 h-3 text-purple-500 hover:text-purple-600" />
					{/if}
				</div>
			</div>
		{/each}
	</div>
</div>
