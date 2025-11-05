<script module lang="ts">
	import Button from '$components/Button.svelte';
	import VirtualList from '$lib/components/VirtualList.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	let items = $state(Array.from({ length: 10 }, (_, i) => `item-${i + 1}`));
	const defaultHeight = 150;

	const { Story } = defineMeta({
		title: 'VirtualList',
		component: VirtualList,
		args: {
			items
		}
	});

	let container = $state<HTMLDivElement>();
</script>

<script lang="ts">
</script>

<Story name="Initial bottom">
	{#snippet template(args)}
		<div class="container" bind:this={container}>
			<VirtualList {...args}>
				{#snippet chunkTemplate(chunk)}
					{#each chunk as item}
						<div class="item" style:height={defaultHeight + 'px'}>
							{item || 'empty'}
						</div>
					{/each}
				{/snippet}
			</VirtualList>
		</div>
		<div class="actions">
			<Button
				onclick={() => {
					items.push('new item ' + (items.length + 1));
				}}
			>
				Add item
			</Button>
			<Button
				onclick={() => {
					const items = Array.from(container?.querySelectorAll('.item') || []);
					const lastElement = items.at(-1);
					if (lastElement && lastElement instanceof HTMLDivElement) {
						lastElement.style.height = 2 * defaultHeight + 'px';
					}
				}}
			>
				Expand last
			</Button>
		</div>
	{/snippet}
</Story>

<style>
	.container {
		display: flex;
		height: 280px;
	}

	.actions {
		display: flex;
		padding: 6px 0;
		gap: 12px;
	}

	.item {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 300px;
		height: 150px;
		border: 1px solid lightgrey;
	}
</style>
