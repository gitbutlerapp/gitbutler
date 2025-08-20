<script module lang="ts">
	import Button from '$components/Button.svelte';
	import ContextMenu from '$components/ContextMenu.svelte';
	import ContextMenuItem from '$components/ContextMenuItem.svelte';
	import ContextMenuItemSubmenu from '$components/ContextMenuItemSubmenu.svelte';
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
					icon="commit"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
					}}
					keyboardShortcut="⌘+Enter"
				/>
				<ContextMenuItem
					label="Commit"
					icon="text-width"
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
				<ContextMenuItem
					label="Revert"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
					}}
				/>
				<ContextMenuItem
					label="Squash"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
					}}
				/>
			</ContextMenuSection>
			<ContextMenuSection title="Danger zone">
				<ContextMenuItem
					label="Delete"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('Commit and bleep');
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
				<ContextMenuItem label="Commit" onclick={() => console.log('Commit')} />
				<ContextMenuItem label="Push" onclick={() => console.log('Push')} />
				<ContextMenuItem label="Pull" onclick={() => console.log('Pull')} />
				<ContextMenuItem label="Fetch" onclick={() => console.log('Fetch')} />
				<ContextMenuItem label="Merge" onclick={() => console.log('Merge')} />
			</ContextMenuSection>
			<ContextMenuSection title="Branch operations">
				<ContextMenuItem label="Create branch" onclick={() => console.log('Create branch')} />
				<ContextMenuItem label="Switch branch" onclick={() => console.log('Switch branch')} />
				<ContextMenuItem label="Delete branch" onclick={() => console.log('Delete branch')} />
				<ContextMenuItem label="Rename branch" onclick={() => console.log('Rename branch')} />
				<ContextMenuItem label="Compare branches" onclick={() => console.log('Compare branches')} />
			</ContextMenuSection>
			<ContextMenuSection title="File operations">
				<ContextMenuItem label="Stage changes" onclick={() => console.log('Stage changes')} />
				<ContextMenuItem label="Unstage changes" onclick={() => console.log('Unstage changes')} />
				<ContextMenuItem label="Discard changes" onclick={() => console.log('Discard changes')} />
				<ContextMenuItem label="View diff" onclick={() => console.log('View diff')} />
				<ContextMenuItem label="Blame" onclick={() => console.log('Blame')} />
			</ContextMenuSection>
			<ContextMenuSection title="Advanced">
				<ContextMenuItem
					label="Interactive rebase"
					onclick={() => console.log('Interactive rebase')}
				/>
				<ContextMenuItem label="Cherry pick" onclick={() => console.log('Cherry pick')} />
				<ContextMenuItem label="Bisect" onclick={() => console.log('Bisect')} />
				<ContextMenuItem label="Reflog" onclick={() => console.log('Reflog')} />
				<ContextMenuItem label="Worktree" onclick={() => console.log('Worktree')} />
			</ContextMenuSection>
			<ContextMenuSection title="Repository">
				<ContextMenuItem label="Clone" onclick={() => console.log('Clone')} />
				<ContextMenuItem label="Fork" onclick={() => console.log('Fork')} />
				<ContextMenuItem label="Archive" onclick={() => console.log('Archive')} />
				<ContextMenuItem label="Settings" onclick={() => console.log('Settings')} />
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
				<ContextMenuItem label="Item 1" onclick={() => console.log('Item 1')} />
				<ContextMenuItem label="Item 2" onclick={() => console.log('Item 2')} />
				<ContextMenuItem label="Item 3" onclick={() => console.log('Item 3')} />
				<ContextMenuItem label="Item 4" onclick={() => console.log('Item 4')} />
				<ContextMenuItem label="Item 5" onclick={() => console.log('Item 5')} />
				<ContextMenuItem label="Item 6" onclick={() => console.log('Item 6')} />
				<ContextMenuItem label="Item 7" onclick={() => console.log('Item 7')} />
				<ContextMenuItem label="Item 8" onclick={() => console.log('Item 8')} />
				<ContextMenuItem label="Item 9" onclick={() => console.log('Item 9')} />
				<ContextMenuItem label="Item 10" onclick={() => console.log('Item 10')} />
				<ContextMenuItem label="Item 11" onclick={() => console.log('Item 11')} />
				<ContextMenuItem label="Item 12" onclick={() => console.log('Item 12')} />
				<ContextMenuItem label="Item 13" onclick={() => console.log('Item 13')} />
				<ContextMenuItem label="Item 14" onclick={() => console.log('Item 14')} />
				<ContextMenuItem label="Item 15" onclick={() => console.log('Item 15')} />
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
				<ContextMenuItem label="Copy" onclick={() => console.log('Copy')} />
				<ContextMenuItem label="Paste" onclick={() => console.log('Paste')} />
				<ContextMenuItem label="Cut" onclick={() => console.log('Cut')} />
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
								}}
							/>
							<ContextMenuItem
								label="Italic"
								onclick={() => {
									console.log('Italic');
									close();
								}}
							/>
							<ContextMenuItem
								label="Underline"
								onclick={() => {
									console.log('Underline');
									close();
								}}
							/>
						</ContextMenuSection>
						<ContextMenuSection title="Alignment">
							<ContextMenuItem
								label="Left"
								onclick={() => {
									console.log('Left');
									close();
								}}
							/>
							<ContextMenuItem
								label="Center"
								onclick={() => {
									console.log('Center');
									close();
								}}
							/>
							<ContextMenuItem
								label="Right"
								onclick={() => {
									console.log('Right');
									close();
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
								}}
							/>
							<ContextMenuItem
								label="Link"
								onclick={() => {
									console.log('Link');
									close();
								}}
							/>
							<ContextMenuItem
								label="Table"
								onclick={() => {
									console.log('Table');
									close();
								}}
							/>
						</ContextMenuSection>
					{/snippet}
				</ContextMenuItemSubmenu>
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
