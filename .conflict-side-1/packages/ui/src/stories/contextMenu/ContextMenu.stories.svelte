<script module lang="ts">
	import Button from '$lib/Button.svelte';
	import ContextMenu from '$lib/ContextMenu.svelte';
	import ContextMenuItem from '$lib/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/ContextMenuSection.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

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
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
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
{/snippet}

<Story name="Left click" />

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
