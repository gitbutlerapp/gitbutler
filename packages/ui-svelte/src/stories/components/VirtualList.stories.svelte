<script module lang="ts">
	import Button from "$components/Button.svelte";
	import VirtualList from "$lib/components/VirtualList.svelte";
	import AsyncContent from "$lib/helpers/AsyncContent.svelte";
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import type { Component, ComponentProps } from "svelte";

	let items = $state(Array.from({ length: 10 }, (_, i) => `item-${i + 1}`));
	const defaultHeight = 150;

	const { Story } = defineMeta({
		title: "VirtualList",
		component: VirtualList as Component<ComponentProps<VirtualList<string>>>,
		args: {
			items,
			visibility: "hover",
			defaultHeight,
			getId: (item?: string) => item,
		},
	});

	let container = $state<HTMLDivElement>();
	let selector = $state<HTMLSelectElement>();
</script>

<script lang="ts">
	let toggle = $state(false);
	let virtualList = $state<VirtualList<any>>();

	function randomDelay(maxMs: number) {
		return Math.round(Math.random() * maxMs);
	}
</script>

<Story name="Initial bottom">
	{#snippet template(args)}
		<div class="container" bind:this={container}>
			<VirtualList bind:this={virtualList} {...args}>
				{#snippet template(item)}
					{@const delay = randomDelay(500)}
					<div class="item" style:min-height={defaultHeight + "px"}>
						{item || "empty"}
						<AsyncContent {delay}>
							<div class="async-content">async content delay: {delay}</div>
						</AsyncContent>
					</div>
				{/snippet}
				{#if toggle}
					<div class="child-content">Hello world!</div>
				{/if}
			</VirtualList>
		</div>
		<div class="actions">
			<Button
				onclick={() => {
					setTimeout(() => {
						items.push("new item " + (items.length + 1));
					}, 2);
					toggle = !toggle;
				}}
			>
				Do both
			</Button>
			<Button
				onclick={() => {
					items.push("new item " + (items.length + 1));
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
					const items = Array.from(container?.querySelectorAll(".item") || []);
					const lastElement = items.at(-1);
					if (lastElement && lastElement instanceof HTMLDivElement) {
						lastElement.style.height = 2 * defaultHeight + "px";
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
				{#snippet template(item)}
					<div class="item" style="min-height: 150px;">
						<div>{item}</div>
						<AsyncContent delay={200}>
							<div class="async-content">async content</div>
						</AsyncContent>
					</div>
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
				virtualList.jumpToIndex(parseInt(selector.value));
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

	.child-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 200px;
		padding: 12px;
		border: 1px solid lightgrey;
	}
</style>
