<script module lang="ts">
	import ContextMenu from '$components/ContextMenu.svelte';
	import ContextMenuItem from '$components/ContextMenuItem.svelte';
	import ContextMenuSection from '$components/ContextMenuSection.svelte';
	import KebabButton from '$components/KebabButton.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Controls / KebabButton',
		component: KebabButton
	});
</script>

<script lang="ts">
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabButtonElement = $state<HTMLElement>();
	let contextElement = $state<HTMLElement>();
</script>

<Story name="Default">
	{#snippet template(args)}
		<div class="demo-container">
			<div class="demo-item" bind:this={contextElement}>
				<span>Click the kebab button or right-click anywhere to open the menu</span>
				<KebabButton
					bind:el={kebabButtonElement}
					{contextElement}
					onclick={() => {
						contextMenu?.open();
					}}
					oncontext={(e) => {
						contextMenu?.open(e);
					}}
					{...args}
				/>
			</div>

			<ContextMenu
				bind:this={contextMenu}
				leftClickTrigger={kebabButtonElement}
				rightClickTrigger={contextElement}
			>
				<ContextMenuSection>
					<ContextMenuItem
						label="Edit"
						onclick={() => {
							// eslint-disable-next-line no-console
							console.log('Edit clicked');
							contextMenu?.close();
						}}
					/>
					<ContextMenuItem
						label="Duplicate"
						onclick={() => {
							// eslint-disable-next-line no-console
							console.log('Duplicate clicked');
							contextMenu?.close();
						}}
					/>
					<ContextMenuItem
						label="Share"
						onclick={() => {
							// eslint-disable-next-line no-console
							console.log('Share clicked');
							contextMenu?.close();
						}}
					/>
				</ContextMenuSection>
				<ContextMenuSection title="Danger zone">
					<ContextMenuItem
						label="Delete"
						onclick={() => {
							// eslint-disable-next-line no-console
							console.log('Delete clicked');
							contextMenu?.close();
						}}
					/>
				</ContextMenuSection>
			</ContextMenu>
		</div>
	{/snippet}
</Story>

<style>
	.demo-container {
		display: flex;
		flex-direction: column;
		max-width: 400px;
		padding: 20px;
		gap: 16px;
	}

	.demo-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		transition: background-color var(--transition-fast);
	}

	.demo-item:hover {
		background-color: var(--clr-bg-1-muted);
	}
</style>
