<script module lang="ts">
	import Button from '$components/Button.svelte';
	import ContextMenu from '$components/ContextMenu.svelte';
	import ContextMenuItem from '$components/ContextMenuItem.svelte';
	import ContextMenuSection from '$components/ContextMenuSection.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Overlays / ContextMenu',
		args: {},
		argTypes: {
			side: {
				options: ['top', 'bottom', 'left', 'right'],
				control: {
					type: 'select'
				}
			},
			verticalAlign: {
				options: ['top', 'bottom'],
				control: {
					type: 'select'
				}
			},
			horizontalAlign: {
				options: ['left', 'right'],
				control: {
					type: 'select'
				}
			}
		}
	});

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let contextTrigger = $state<HTMLButtonElement | undefined>();
</script>

<script lang="ts">
</script>

<Story name="Left click">
	{#snippet template(args)}
		<div class="wrap">
			<Button
				kind="outline"
				bind:el={contextTrigger}
				onclick={() => {
					contextMenu?.toggle();
				}}>Toggle context menu</Button
			>
		</div>

		<ContextMenu bind:this={contextMenu} leftClickTrigger={contextTrigger} {...args}>
			<ContextMenuSection>
				<ContextMenuItem
					label="Commit and bleep"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
					}}
				/>
				<ContextMenuItem
					label="Commit"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
					}}
				/>
			</ContextMenuSection>
			<ContextMenuSection title="More">
				<ContextMenuItem
					label="Another commit"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
					}}
				/>
				<ContextMenuItem
					label="Amend"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
					}}
				/>
			</ContextMenuSection>
		</ContextMenu>
	{/snippet}
</Story>

<style>
	.wrap {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 600px;
		border-radius: var(--radius-ml);
		background: var(--clr-bg-2);
	}
</style>
