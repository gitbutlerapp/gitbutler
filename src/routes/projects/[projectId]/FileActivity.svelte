<script lang="ts">
	import type { Delta } from '$lib/api';
	import { fillBuckets, type Bucket } from './histogram';

	export let deltas: Delta[];
	export let buckets: Bucket[];

	$: groups = fillBuckets(
		deltas.map((delta) => delta.timestampMs),
		buckets
	);

	$: largestGroup = Math.max(...groups.map((group) => group.length));
</script>

<div class="file-activity w-full font-mono text-zinc-400">
	{#each groups as group}
		<span
			class={`inline-block w-full rounded-t-sm`}
			style="
			height: {Math.round((group.length / largestGroup) * 100)}%;
			background: linear-gradient(0deg, #3b82f6 0%, #9565FF {100 -
				Math.round((group.length / largestGroup) * 100) +
				100}%);
			"
		/>
	{/each}
</div>

<style lang="postcss">
	.file-activity {
		@apply flex h-full items-baseline gap-1 px-4 pt-2;
		align-items: flex-end;
		border-bottom: 1px solid rgb(55, 55, 55);
		background-color: rgba(0, 0, 0, 0.1);
	}
</style>
