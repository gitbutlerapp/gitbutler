<script module lang="ts">
	import FileListItem from '$components/file/FileListItem.svelte';
	import FolderListItem from '$components/file/FolderListItem.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'List items / FolderListItem',
		args: {
			checkboxes: false
		}
	});
</script>

<script lang="ts">
</script>

<Story name="default">
	{#snippet template(args)}
		<div class="wrapper">
			<FolderListItem
				name="Editor"
				showCheckbox={args.checkboxes}
				checked={false}
				isExpanded={true}
				indeterminate={false}
				depth={0}
			/>
			<FileListItem
				showCheckbox={args.checkboxes}
				filePath="src/components/FileTreeNode.svelte"
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
			<FileListItem
				showCheckbox={args.checkboxes}
				depth={2}
				filePath="src/components/List.svelte"
				draggable
				listMode="tree"
			/>
			<FileListItem
				showCheckbox={args.checkboxes}
				depth={2}
				filePath="src/components/lib.js"
				draggable
				listMode="tree"
			/>
		</div>
	{/snippet}
</Story>

<Story name="Tree view" />

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		max-width: 300px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
</style>
