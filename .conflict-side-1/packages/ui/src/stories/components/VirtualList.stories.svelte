<script module lang="ts">
	import Button from '$components/Button.svelte';
	import VirtualList from '$lib/components/VirtualList.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import type { Component, ComponentProps } from 'svelte';

	let items = $state(Array.from({ length: 10 }, (_, i) => `item-${i + 1}`));
	const defaultHeight = 150;

	const { Story } = defineMeta({
		title: 'VirtualList',
		component: VirtualList as Component<ComponentProps<VirtualList<string>>>,
		args: {
			items,
			batchSize: 1,
			visibility: 'hover',
			defaultHeight
		}
	});

	let container = $state<HTMLDivElement>();
</script>

<script lang="ts">
	let toggle = $state(false);
	let virtualList = $state<VirtualList<any>>();
</script>

<Story name="Initial bottom">
	{#snippet template(args)}
		<div class="container" bind:this={container}>
			<VirtualList bind:this={virtualList} {...args}>
				{#snippet chunkTemplate(chunk)}
					{#each chunk as item}
						<div class="item" style:height={defaultHeight + 'px'}>
							{item || 'empty'}
						</div>
					{/each}
				{/snippet}
				{#if toggle}
					Hello world!
				{/if}
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
					toggle = !toggle;
				}}
			>
				Add temp item
			</Button>
			<Button
				onclick={() => {
					virtualList?.scrollToBottom();
				}}
			>
				Bottom
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
		height: 320px;
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
		border: 1px solid lightgrey;
	}
</style>
