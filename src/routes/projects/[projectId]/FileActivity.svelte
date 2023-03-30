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

<div class="font-mono text-zinc-400">
	{#each buckets as bucket}
		{#if bucket < 1}
			<span class="text-zinc-600">▁</span>
		{:else if bucket < 2}
			<span class="text-blue-200">▂</span>
		{:else if bucket < 3}
			<span class="text-blue-200">▃</span>
		{:else if bucket < 4}
			<span class="text-blue-200">▄</span>
		{:else if bucket < 5}
			<span class="text-blue-200">▅</span>
		{:else if bucket < 6}
			<span class="text-blue-200">▆</span>
		{:else if bucket < 6}
			<span class="text-blue-200">▇</span>
		{:else}
			<span class="text-blue-200">█</span>
		{/if}
	{/each}
</div>
