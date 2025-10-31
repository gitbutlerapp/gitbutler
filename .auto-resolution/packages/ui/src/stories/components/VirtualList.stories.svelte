<script context="module" lang="ts">
	import VirtualList from '$lib/components/VirtualList.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const items = Array.from({ length: 20 }, (_, i) => `item-${i + 1}`);

	const { Story } = defineMeta({
		title: 'VirtualList',
		component: VirtualList<string>,
		args: {
			items,
			batchSize: 1
		}
	});
</script>

<script lang="ts">
</script>

<Story name="VirtualList">
	{#snippet template(args)}
		<div class="container">
			<VirtualList {...args} initialPosition="bottom" stickToBottom>
				{#snippet chunkTemplate(chunk)}
					{#each chunk as item}
						<div class="item">
							{item}
						</div>
					{/each}
				{/snippet}
			</VirtualList>
		</div>
	{/snippet}
</Story>

<style>
	.container {
		display: flex;
		height: 320px;
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
