<script module lang="ts">
	import FileListItemV3 from '$lib/file/FileListItemV3.svelte';
	import FolderListItem from '$lib/file/FolderListItem.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'List items / FolderListItem',
		args: {
			checkboxes: false
		}
	});
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<div class="wrapper">
		<FolderListItem
			name="Editor"
			showCheckbox={args.checkboxes}
			checked={false}
			isExpanded={true}
			indeterminate={false}
			depth={0}
		/>
		<FileListItemV3
			showCheckbox={args.checkboxes}
			filePath="src/components/v3/FileTreeNode.svelte"
			draggable
			listMode="tree"
			depth={1}
		/>
		<FolderListItem
			name="components"
			showCheckbox={args.checkboxes}
			checked={false}
			isExpanded={true}
			indeterminate={false}
			depth={1}
		/>
		<FileListItemV3
			showCheckbox={args.checkboxes}
			depth={2}
			filePath="src/components/v3/List.svelte"
			draggable
			listMode="tree"
		/>
		<FileListItemV3
			showCheckbox={args.checkboxes}
			depth={2}
			filePath="src/components/v3/lib.js"
			draggable
			listMode="tree"
		/>
	</div>
{/snippet}

<Story name="Tree view" />

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		max-width: 300px;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
	}
</style>
