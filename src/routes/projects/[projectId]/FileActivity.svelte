<script lang="ts">
	import type { Delta } from '$lib/deltas';

	export let deltas: Delta[];

	const timestamps = deltas.map((delta) => delta.timestampMs).sort((a, b) => a - b);

	const totalBuckets = 18;
	const range = timestamps[timestamps.length - 1] - timestamps[0];
	const bucketSize = range / totalBuckets;
	const buckets: number[] = Array.from({ length: totalBuckets }, () => 0);

	timestamps.forEach((timestamp) => {
		const bucketIndex = Math.floor((timestamp - timestamps[0]) / bucketSize);
		buckets[bucketIndex] += 1;
	});
</script>

<div class="file-activity font-mono text-zinc-400 w-[50%]">
	{#each buckets as bucket}
		{#if bucket < 1}
			<span class="file-bar bar bar-0 text-zinc-600 "></span>
		{:else if bucket < 2}
			<span class="file-bar bar bar-1 text-blue-500"></span>
		{:else if bucket < 3}
			<span class="file-bar bar bar-2 text-blue-500"></span>
		{:else if bucket < 4}
			<span class="file-bar bar bar-3 text-blue-500"></span>
		{:else if bucket < 5}
			<span class="file-bar bar bar-4 text-blue-500"></span>
		{:else if bucket < 6}
			<span class="file-bar bar bar-5 text-blue-500"></span>
		{:else if bucket < 7}
			<span class="file-bar bar bar-6 text-blue-500"></span>
		{:else}
			<span class="file-bar bar bar-7 text-blue-500"></span>
		{/if}
	{/each}
</div>


<style>
	.file-activity {
		@apply h-full flex items-baseline gap-1 pt-1 pl-1 pr-1;
		align-items: flex-end;
		border-bottom: 1px solid rgb(55, 55, 55);
		background-color: rgba(0, 0, 0, 0.1);
	}
	.file-bar {
		width: auto;
	}
	.bar {
		display: inline-block;
		width: 100%;
		height: 1rem;
		background-color: #6959DF ;
		border-radius: 2px 2px 0 0;

	}
	.bar-0 {
		height: 0;
	}
	.bar-1 {
		height: 14%;
	}
	.bar-2 {
		height: 28%;
	}
	.bar-3 {
		height: 42%;
	}
	.bar-4 {
		height: 56%;
	}
	.bar-5 {
		height: 70%;
	}
	.bar-6 {
		height: 84%;
	}
	.bar-7 {
		height: 100%;
	}
</style>