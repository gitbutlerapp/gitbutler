<script module lang="ts">
	import ContextMenuItem from '$components/ContextMenuItem.svelte';
	import ContextMenuSection from '$components/ContextMenuSection.svelte';
	import DropdownButton from '$components/DropdownButton.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / DropdownButton',
		component: DropdownButton
	});
</script>

<Story name="Default">
	{#snippet template()}
		<div class="wrapper">
			<DropdownButton>
				Action
				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Option 1"
							onclick={() => {
								// eslint-disable-next-line no-console
								console.log('Option 1 clicked');
							}}
						/>
						<ContextMenuItem
							label="Option 2"
							onclick={() => {
								// eslint-disable-next-line no-console
								console.log('Option 2 clicked');
							}}
						/>
						<ContextMenuItem
							label="Option 3"
							onclick={() => {
								// eslint-disable-next-line no-console
								console.log('Option 3 clicked');
							}}
						/>
					</ContextMenuSection>
				{/snippet}
			</DropdownButton>
		</div>
	{/snippet}
</Story>

<style>
	.wrapper {
		display: flex;
		align-items: center;
		padding: 20px;
		gap: 10px;
	}
</style>
