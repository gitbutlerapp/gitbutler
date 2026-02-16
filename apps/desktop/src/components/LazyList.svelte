<script lang="ts" module>
	type T = any;

	export interface ItemContext {
		index: number;
		first: boolean;
		last: boolean;
	}
</script>

<script lang="ts" generics="T">
	/**
	 * Lazily renders a list of many many items. This is intended to be used
	 * in contexts where simply rendering the quantity of items causes the DOM
	 * to have poor performance, rather than computing the initial list being
	 * the blocker.
	 */

	import LazyloadContainer from "$components/LazyloadContainer.svelte";
	import { type Snippet } from "svelte";

	interface Props {
		items: T[];
		template: Snippet<[T, ItemContext]>;
		chunkSize?: number;
	}

	const { items, template, chunkSize = 20 }: Props = $props();

	let displayCount = $derived(Math.min(chunkSize, items.length));

	function loadMore() {
		if (displayCount >= items.length) return;
		displayCount = Math.min(displayCount + chunkSize, items.length);
	}
</script>

{#if items.length > 0}
	<LazyloadContainer
		minTriggerCount={Math.min(items.length, chunkSize - 5)}
		ontrigger={() => {
			loadMore();
		}}
	>
		{#each items.slice(0, displayCount) as item, index}
			{@render template(item, {
				index,
				first: index === 0,
				last: index === items.length - 1,
			})}
		{/each}
	</LazyloadContainer>
{/if}
