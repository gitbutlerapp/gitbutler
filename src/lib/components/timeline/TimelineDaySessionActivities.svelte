<script lang="ts">
	import type { Activity } from '$lib/sessions';
	import {
		IconCircleHalf,
		IconMapPinFilled,
		IconSquareRoundedFilled,
		IconCircleFilled
	} from '$lib/components/icons';

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
	<hr class="z-0 h-px border-0 bg-slate-400" />
	<div class="absolute inset-0 -mt-1.5">
		{#each activities as activity}
			<div
				class="-mx-1.5 flex"
				style="position:relative; left: {proportionOfTime(activity.timestampMs)}%;"
			>
				<div
					class="absolute inset-0 z-50 h-3 w-3 text-slate-700"
					style=""
					title="{activity.type}: {activity.message} at {toHumanReadableTime(activity.timestampMs)}"
				>
					{#if activity.type.startsWith('commit')}
						<IconSquareRoundedFilled class="h-3 w-3 text-sky-500 hover:text-sky-600" />
					{:else if activity.type.startsWith('merge')}
						<IconMapPinFilled class="h-3 w-3 text-green-500 hover:text-green-600" />
					{:else if activity.type.startsWith('rebase')}
						<IconCircleHalf class="h-3 w-3 text-orange-500 hover:text-orange-600" />
					{:else if activity.type.startsWith('push')}
						<IconCircleFilled class="h-3 w-3 text-purple-500 hover:text-purple-600" />
					{/if}
				</div>
			</div>
		{/each}
	</div>
</div>
