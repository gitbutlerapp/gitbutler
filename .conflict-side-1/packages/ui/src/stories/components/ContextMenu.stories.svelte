<script module lang="ts">
	import Button from '$components/Button.svelte';
	import ContextMenu from '$components/ContextMenu.svelte';
	import ContextMenuItem from '$components/ContextMenuItem.svelte';
	import ContextMenuItemSubmenu from '$components/ContextMenuItemSubmenu.svelte';
	import ContextMenuSection from '$components/ContextMenuSection.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Overlays / ContextMenu',
		args: {
			side: 'bottom',
			align: 'start',
			offset: 0
		},
		argTypes: {
			side: {
				control: { type: 'select' },
				options: ['top', 'bottom', 'left', 'right'],
				description: 'Which side of the trigger element to position the menu'
			},
			align: {
				control: { type: 'select' },
				options: ['start', 'center', 'end'],
				description: 'How to align the menu relative to the trigger element'
			},
			offset: {
				control: { type: 'number' },
				description: 'Distance in pixels from the trigger element'
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
					icon="commit"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
						contextMenu?.close();
					}}
					keyboardShortcut="⌘+Enter"
				/>
				<ContextMenuItem
					label="Commit"
					icon="text-width"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
						contextMenu?.close();
					}}
				/>
			</ContextMenuSection>
			<ContextMenuSection title="More">
				<ContextMenuItem
					label="Another commit"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Amend"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Revert"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Squash"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
						contextMenu?.close();
					}}
				/>
			</ContextMenuSection>
			<ContextMenuSection title="Danger zone">
				<ContextMenuItem
					label="Delete"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
						contextMenu?.close();
					}}
				/>
			</ContextMenuSection>
		</ContextMenu>
	{/snippet}
</Story>

<!-- eslint-disable no-console -->
<Story name="Right-click">
	{#snippet template(args)}
		<div class="wrap">
			<Button
				kind="outline"
				bind:el={contextTrigger}
				oncontextmenu={(e) => {
					e.preventDefault();
					contextMenu?.toggle(e);
				}}>Right-click for scrollable menu</Button
			>
		</div>

		<ContextMenu bind:this={contextMenu} rightClickTrigger={contextTrigger} {...args}>
			<ContextMenuSection title="Quick actions">
				<ContextMenuItem
					label="Commit"
					onclick={() => {
						console.log('Commit');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Push"
					onclick={() => {
						console.log('Push');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Pull"
					onclick={() => {
						console.log('Pull');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Fetch"
					onclick={() => {
						console.log('Fetch');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Merge"
					onclick={() => {
						console.log('Merge');
						contextMenu?.close();
					}}
				/>
			</ContextMenuSection>
			<ContextMenuSection title="Branch operations">
				<ContextMenuItem
					label="Create branch"
					onclick={() => {
						console.log('Create branch');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Switch branch"
					onclick={() => {
						console.log('Switch branch');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Delete branch"
					onclick={() => {
						console.log('Delete branch');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Rename branch"
					onclick={() => {
						console.log('Rename branch');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Compare branches"
					onclick={() => {
						console.log('Compare branches');
						contextMenu?.close();
					}}
				/>
			</ContextMenuSection>
			<ContextMenuSection>
				<ContextMenuItem
					label="Stage changes"
					onclick={() => {
						console.log('Stage changes');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Unstage changes"
					onclick={() => {
						console.log('Unstage changes');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Discard changes"
					onclick={() => {
						console.log('Discard changes');
						contextMenu?.close();
					}}
				/>
			</ContextMenuSection>
			<ContextMenuSection>
				<ContextMenuItem
					label="Bisect"
					onclick={() => {
						console.log('Bisect');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Reflog"
					onclick={() => {
						console.log('Reflog');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Worktree"
					onclick={() => {
						console.log('Worktree');
						contextMenu?.close();
					}}
				/>
			</ContextMenuSection>
			<ContextMenuSection>
				<ContextMenuItem
					label="Clone"
					onclick={() => {
						console.log('Clone');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Fork"
					onclick={() => {
						console.log('Fork');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Archive"
					onclick={() => {
						console.log('Archive');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Settings"
					onclick={() => {
						console.log('Settings');
						contextMenu?.close();
					}}
				/>
			</ContextMenuSection>
		</ContextMenu>
	{/snippet}
</Story>

<!-- eslint-disable no-console -->
<Story name="Bottom positioned (test flickering fix)">
	{#snippet template(args)}
		<div class="wrap" style="margin-top: 200px;">
			<Button
				kind="outline"
				bind:el={contextTrigger}
				onclick={() => {
					contextMenu?.toggle();
				}}>Test flickering fix</Button
			>
		</div>

		<ContextMenu bind:this={contextMenu} leftClickTrigger={contextTrigger} side="bottom" {...args}>
			<ContextMenuSection title="Many items to force scrolling">
				<ContextMenuItem
					label="Item 1"
					onclick={() => {
						console.log('Item 1');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 2"
					onclick={() => {
						console.log('Item 2');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 3"
					onclick={() => {
						console.log('Item 3');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 4"
					onclick={() => {
						console.log('Item 4');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 5"
					onclick={() => {
						console.log('Item 5');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 6"
					onclick={() => {
						console.log('Item 6');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 7"
					onclick={() => {
						console.log('Item 7');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 8"
					onclick={() => {
						console.log('Item 8');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 9"
					onclick={() => {
						console.log('Item 9');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 10"
					onclick={() => {
						console.log('Item 10');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 11"
					onclick={() => {
						console.log('Item 11');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 12"
					onclick={() => {
						console.log('Item 12');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 13"
					onclick={() => {
						console.log('Item 13');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 14"
					onclick={() => {
						console.log('Item 14');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Item 15"
					onclick={() => {
						console.log('Item 15');
						contextMenu?.close();
					}}
				/>
			</ContextMenuSection>
		</ContextMenu>
	{/snippet}
</Story>

<Story name="With Submenus">
	{#snippet template(args)}
		<div class="wrap">
			<Button
				kind="outline"
				bind:el={contextTrigger}
				onclick={() => {
					contextMenu?.toggle();
				}}>Context menu with submenus</Button
			>
		</div>

		<ContextMenu bind:this={contextMenu} leftClickTrigger={contextTrigger} side="bottom" {...args}>
			<ContextMenuSection title="Basic actions">
				<ContextMenuItem
					label="Copy"
					onclick={() => {
						console.log('Copy');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Paste"
					onclick={() => {
						console.log('Paste');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Cut"
					onclick={() => {
						console.log('Cut');
						contextMenu?.close();
					}}
				/>
			</ContextMenuSection>
			<ContextMenuSection title="Advanced">
				<ContextMenuItemSubmenu label="Format" icon="text-bold">
					{#snippet submenu({ close })}
						<ContextMenuSection>
							<ContextMenuItem
								label="Bold"
								onclick={() => {
									console.log('Bold');
									close();
									contextMenu?.close();
								}}
							/>
							<ContextMenuItem
								label="Italic"
								onclick={() => {
									console.log('Italic');
									close();
									contextMenu?.close();
								}}
							/>
							<ContextMenuItem
								label="Underline"
								onclick={() => {
									console.log('Underline');
									close();
									contextMenu?.close();
								}}
							/>
						</ContextMenuSection>
						<ContextMenuSection title="Alignment">
							<ContextMenuItem
								label="Left"
								onclick={() => {
									console.log('Left');
									close();
									contextMenu?.close();
								}}
							/>
							<ContextMenuItem
								label="Center"
								onclick={() => {
									console.log('Center');
									close();
									contextMenu?.close();
								}}
							/>
							<ContextMenuItem
								label="Right"
								onclick={() => {
									console.log('Right');
									close();
									contextMenu?.close();
								}}
							/>
						</ContextMenuSection>
					{/snippet}
				</ContextMenuItemSubmenu>
				<ContextMenuItemSubmenu label="Insert" icon="plus">
					{#snippet submenu({ close })}
						<ContextMenuSection>
							<ContextMenuItem
								label="Image"
								onclick={() => {
									console.log('Image');
									close();
									contextMenu?.close();
								}}
							/>
							<ContextMenuItem
								label="Link"
								onclick={() => {
									console.log('Link');
									close();
									contextMenu?.close();
								}}
							/>
							<ContextMenuItem
								label="Table"
								onclick={() => {
									console.log('Table');
									close();
									contextMenu?.close();
								}}
							/>
						</ContextMenuSection>
					{/snippet}
				</ContextMenuItemSubmenu>
			</ContextMenuSection>
		</ContextMenu>
	{/snippet}
</Story>

<Story name="With caption">
	{#snippet template(args)}
		<div class="wrap">
			<Button
				kind="outline"
				bind:el={contextTrigger}
				onclick={() => {
					contextMenu?.toggle();
				}}>Context menu with caption</Button
			>
		</div>

		<ContextMenu bind:this={contextMenu} leftClickTrigger={contextTrigger} side="bottom" {...args}>
			<ContextMenuSection title="Item with caption">
				<ContextMenuItem
					label="Rebase"
					caption="Move your commits on top of upstream changes. Creates clean, linear history."
					onclick={() => {
						console.log('Copy');
						contextMenu?.close();
					}}
				/>
				<ContextMenuItem
					label="Paste"
					caption="Paste from clipboard"
					onclick={() => {
						console.log('Paste');
						contextMenu?.close();
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
		padding: 400px 0 100px;
		border-radius: var(--radius-ml);
		background: var(--clr-bg-2);
	}
</style>
