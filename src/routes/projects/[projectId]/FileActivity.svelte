<script lang="ts">
	import type { Delta } from '$lib/deltas';
	import { bucketByTimestamp } from './histogram';

	export let deltas: Delta[];
	export let largestBucketSize: number;

	$: buckets = bucketByTimestamp(
		deltas.map((delta) => delta.timestampMs),
		18
	);
</script>

<div class="file-activity w-[50%] font-mono text-zinc-400">
	{#each buckets as bucket}
		<span
			class={`inline-block w-full rounded-t-sm`}
			style="
			height: {Math.round((bucket.length / largestBucketSize) * 100)}%;
			background: #3b82f6;
			background: linear-gradient(0deg, #3b82f6 0%, #eab308 {100 -
				Math.round((bucket.length / largestBucketSize) * 100) +
				100}%);
			"
		/>
	{/each}
</div>

<style>
	.file-activity {
		@apply flex h-full items-baseline gap-1 px-4 pt-2;
		align-items: flex-end;
		border-bottom: 1px solid rgb(55, 55, 55);
		background-color: rgba(0, 0, 0, 0.1);
	}
</style>
