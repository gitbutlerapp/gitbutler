<script module lang="ts">
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
	let contextElement = $state<HTMLElement>();
</script>

<Story name="Default">
	{#snippet template(args)}
		<div class="demo-container">
			<div class="demo-item" bind:this={contextElement}>
				<span>Click the kebab button or right-click anywhere to open the menu</span>
				<KebabButton {...args}>
					{#snippet contextMenu({ close })}
						<ContextMenuSection>
							<ContextMenuItem
								label="Edit"
								onclick={() => {
									// eslint-disable-next-line no-console
									console.log('Edit clicked');
									close();
								}}
							/>
							<ContextMenuItem
								label="Duplicate"
								onclick={() => {
									// eslint-disable-next-line no-console
									console.log('Duplicate clicked');
									close();
								}}
							/>
							<ContextMenuItem
								label="Share"
								onclick={() => {
									// eslint-disable-next-line no-console
									console.log('Share clicked');
									close();
								}}
							/>
						</ContextMenuSection>
						<ContextMenuSection title="Danger zone">
							<ContextMenuItem
								label="Delete"
								onclick={() => {
									// eslint-disable-next-line no-console
									console.log('Delete clicked');
									close();
								}}
							/>
						</ContextMenuSection>
					{/snippet}
				</KebabButton>
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Show On Hover">
	{#snippet template(args)}
		<div class="demo-container">
			<div class="demo-item" bind:this={contextElement}>
				<span>Hover over this area to reveal the kebab button</span>
				<KebabButton showOnHover {contextElement} {...args}>
					{#snippet contextMenu({ close })}
						<ContextMenuSection>
							<ContextMenuItem
								label="Edit"
								onclick={() => {
									// eslint-disable-next-line no-console
									console.log('Edit clicked');
									close();
								}}
							/>
							<ContextMenuItem
								label="Delete"
								onclick={() => {
									// eslint-disable-next-line no-console
									console.log('Delete clicked');
									close();
								}}
							/>
						</ContextMenuSection>
					{/snippet}
				</KebabButton>
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Minimal">
	{#snippet template(args)}
		<div class="demo-container">
			<div class="demo-item">
				<span>Always visible minimal kebab button</span>
				<KebabButton minimal {...args}>
					{#snippet contextMenu({ close })}
						<ContextMenuSection>
							<ContextMenuItem
								label="Settings"
								onclick={() => {
									// eslint-disable-next-line no-console
									console.log('Settings clicked');
									close();
								}}
							/>
							<ContextMenuItem
								label="About"
								onclick={() => {
									// eslint-disable-next-line no-console
									console.log('About clicked');
									close();
								}}
							/>
						</ContextMenuSection>
					{/snippet}
				</KebabButton>
			</div>
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
