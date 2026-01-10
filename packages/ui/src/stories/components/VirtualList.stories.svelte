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
	let selector = $state<HTMLSelectElement>();
</script>

<script lang="ts">
	import AsyncContent from '../helpers/AsyncContent.svelte';

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

<Story name="Start index">
	{#snippet template(args)}
		<div class="container" bind:this={container}>
			<VirtualList bind:this={virtualList} {...args} defaultHeight={150} startIndex={4}>
				{#snippet chunkTemplate(chunk)}
					{#each chunk as item}
						<div class="item" style="min-height: 150px;">
							<div>{item}</div>
							<AsyncContent delay={500}>
								<div class="async-content">async content</div>
							</AsyncContent>
						</div>
					{/each}
				{/snippet}
			</VirtualList>
		</div>
		<select bind:this={selector}>
			{#each args.items as _, i}
				<option>
					{i}
				</option>
			{/each}
		</select>
		<button
			type="button"
			onclick={() => {
				if (!virtualList || !selector) return;
				virtualList.scrollToIndex(parseInt(selector.value));
			}}
		>
			goto
		</button>
	{/snippet}
</Story>

<style>
	.container {
		display: flex;
		height: 480px;
	}

	.actions {
		display: flex;
		padding: 6px 0;
		gap: 12px;
	}

	.item {
		display: flex;
		flex-direction: column;
		width: 300px;
		padding: 12px;
		gap: 12px;
		border: 1px solid lightgrey;
	}
	.async-content {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 200px;
		padding: 12px;
		border: 1px solid lightgrey;
	}
</style>
