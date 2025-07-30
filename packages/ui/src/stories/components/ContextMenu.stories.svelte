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
<Story name="Scrollable menu (many items)">
	{#snippet template(args)}
		<div class="wrap">
			<Button
				kind="outline"
				bind:el={contextTrigger}
				onclick={() => {
					contextMenu?.toggle();
				}}>Long context menu</Button
			>
		</div>

		<ContextMenu bind:this={contextMenu} leftClickTrigger={contextTrigger} {...args}>
			<ContextMenuSection title="File operations">
				<ContextMenuItem label="New file" onclick={() => console.log('New file')} />
				<ContextMenuItem label="Open file" onclick={() => console.log('Open file')} />
				<ContextMenuItem label="Save file" onclick={() => console.log('Save file')} />
				<ContextMenuItem label="Save as..." onclick={() => console.log('Save as')} />
				<ContextMenuItem label="Close file" onclick={() => console.log('Close file')} />
			</ContextMenuSection>
			<ContextMenuSection title="Edit operations">
				<ContextMenuItem label="Cut" onclick={() => console.log('Cut')} />
				<ContextMenuItem label="Copy" onclick={() => console.log('Copy')} />
				<ContextMenuItem label="Paste" onclick={() => console.log('Paste')} />
				<ContextMenuItem label="Select all" onclick={() => console.log('Select all')} />
				<ContextMenuItem label="Find" onclick={() => console.log('Find')} />
				<ContextMenuItem label="Replace" onclick={() => console.log('Replace')} />
			</ContextMenuSection>
			<ContextMenuSection title="Git operations">
				<ContextMenuItem label="Commit" onclick={() => console.log('Commit')} />
				<ContextMenuItem label="Commit and push" onclick={() => console.log('Commit and push')} />
				<ContextMenuItem label="Amend commit" onclick={() => console.log('Amend commit')} />
				<ContextMenuItem label="Revert changes" onclick={() => console.log('Revert changes')} />
				<ContextMenuItem label="Stash changes" onclick={() => console.log('Stash changes')} />
				<ContextMenuItem label="Create branch" onclick={() => console.log('Create branch')} />
				<ContextMenuItem label="Merge branch" onclick={() => console.log('Merge branch')} />
				<ContextMenuItem label="Rebase" onclick={() => console.log('Rebase')} />
			</ContextMenuSection>
			<ContextMenuSection title="More actions">
				<ContextMenuItem label="Format code" onclick={() => console.log('Format code')} />
				<ContextMenuItem label="Run tests" onclick={() => console.log('Run tests')} />
				<ContextMenuItem label="Build project" onclick={() => console.log('Build project')} />
				<ContextMenuItem label="Deploy" onclick={() => console.log('Deploy')} />
				<ContextMenuItem label="Settings" onclick={() => console.log('Settings')} />
			</ContextMenuSection>
			<ContextMenuSection title="Danger zone">
				<ContextMenuItem label="Delete file" onclick={() => console.log('Delete file')} />
				<ContextMenuItem label="Reset hard" onclick={() => console.log('Reset hard')} />
				<ContextMenuItem
					label="Delete repository"
					onclick={() => console.log('Delete repository')}
				/>
			</ContextMenuSection>
		</ContextMenu>
	{/snippet}
</Story>

<!-- eslint-disable no-console -->
<Story name="Right-click scrollable menu">
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
				<ContextMenuItem label="Item 16" onclick={() => console.log('Item 16')} />
				<ContextMenuItem label="Item 17" onclick={() => console.log('Item 17')} />
				<ContextMenuItem label="Item 18" onclick={() => console.log('Item 18')} />
				<ContextMenuItem label="Item 19" onclick={() => console.log('Item 19')} />
				<ContextMenuItem label="Item 20" onclick={() => console.log('Item 20')} />
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
