<script lang="ts">
	import type { Activity } from '$lib/sessions';
	import FaSquare from 'svelte-icons/fa/FaSquare.svelte';
	import FaCircle from 'svelte-icons/fa/FaCircle.svelte';
	import FaAdjust from 'svelte-icons/fa/FaAdjust.svelte';
	import FaMapMarker from 'svelte-icons/fa/FaMapMarker.svelte';

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
					{#if activity.type === 'commit'}
						<div class="text-sky-500 hover:text-sky-600">
							<FaSquare />
						</div>
					{:else if activity.type === 'merge'}
						<div class="text-green-500 hover:text-green-600">
							<FaMapMarker />
						</div>
					{:else if activity.type === 'rebase'}
						<div class="text-orange-500 hover:text-orange-600">
							<FaAdjust />
						</div>
					{:else if activity.type === 'push'}
						<div class="text-purple-500 hover:text-purple-600">
							<FaCircle />
						</div>
					{/if}
				</div>
			</div>
		{/each}
	</div>
</div>
