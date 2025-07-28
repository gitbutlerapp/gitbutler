<script lang="ts" module>
	type T = any;
</script>

<script lang="ts" generics="T">
	/**
	 * Lazily renders a list of many many items. This is intended to be used
	 * in contexts where simply rendering the quanity of items causes the DOM
	 * to have poor performance, rather than computing the initial list being
	 * the blocker.
	 */

	import LazyloadContainer from '$components/shared/LazyloadContainer.svelte';
	import { chunk } from '$lib/utils/array';
	import { type Snippet } from 'svelte';

	interface Props {
		items: T[];
		item: Snippet<[T]>;
		chunkSize?: number;
	}

	const { items, item, chunkSize = 20 }: Props = $props();

	let chunkedItems: T[][] = [];
	let displayedItems = $state<T[]>([]);
	let currentDisplayIndex = $state(0);

	// Make sure we display when the file list is reset
	$effect(() => {
		chunkedItems = chunk(items, chunkSize);
		displayedItems = chunkedItems[0] || [];
		currentDisplayIndex = 0;
	});

	function loadMore() {
		if (currentDisplayIndex + 1 >= chunkedItems.length) return;

		currentDisplayIndex += 1;
		const currentChunkedFiles = chunkedItems[currentDisplayIndex] ?? [];
		displayedItems = [...displayedItems, ...currentChunkedFiles];
	}
</script>

{#if items.length > 0}
	<LazyloadContainer
		minTriggerCount={Math.min(items.length, chunkSize - 5)}
		ontrigger={() => {
			loadMore();
		}}
	>
		{#each displayedItems as displayedItem}
			{@render item(displayedItem)}
		{/each}
	</LazyloadContainer>
{/if}
