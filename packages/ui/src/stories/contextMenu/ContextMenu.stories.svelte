<script module lang="ts">
	import Button from '$lib/Button.svelte';
	import ContextMenu from '$lib/ContextMenu.svelte';
	import ContextMenuItem from '$lib/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/ContextMenuSection.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Overlays / ContextMenu',
		args: {},
		argTypes: {}
	});

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let contextTrigger = $state<HTMLButtonElement | undefined>();
</script>

<Story name="Left click">
	<div class="wrap">
		<Button
			kind="outline"
			bind:el={contextTrigger}
			onclick={() => {
				contextMenu?.toggle();
			}}>Toggle context menu</Button
		>
	</div>
</Story>

<ContextMenu bind:this={contextMenu} leftClickTrigger={contextTrigger}>
	<ContextMenuSection>
		<ContextMenuItem
			label="Commit and bleep"
			onclick={() => {
				console.log('Commit and bleep');
			}}
		/>
		<ContextMenuItem
			label="Commit"
			onclick={() => {
				console.log('Commit and bleep');
			}}
		/>
	</ContextMenuSection>
	<ContextMenuSection title="More">
		<ContextMenuItem
			label="Another commit"
			onclick={() => {
				console.log('Commit and bleep');
			}}
		/>
		<ContextMenuItem
			label="Amend"
			onclick={() => {
				console.log('Commit and bleep');
			}}
		/>
	</ContextMenuSection>
</ContextMenu>

<style>
	.wrap {
		display: flex;
		justify-content: center;
		align-items: center;
		height: 600px;
		width: 100%;
		background: var(--clr-bg-2);
		border-radius: var(--radius-ml);
	}
</style>
