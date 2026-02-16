<script module lang="ts">
	import FileListItem from '$components/file/FileListItem.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'List items / FileListItem',
		component: FileListItem
	});
</script>

<Story
	name="Playground"
	args={{
		filePath: 'path/to/components/file.svelte',
		fileStatus: 'addition',
		fileStatusStyle: 'dot',
		pathFirst: true,
		clickable: true,
		selected: false,
		conflicted: false,
		draggable: true,
		showCheckbox: true,
		checked: true,
		lockText: 'Locked by someone',
		onclick: () => {
			// eslint-disable-next-line no-console
			console.log('clicked');
		},
		oncheck: (e: Event) => {
			// eslint-disable-next-line no-console
			console.log('checked', e);
		}
	}}
/>

<Story
	name="Resolve conflict"
	args={{
		filePath: '/path/to/file.svelte',
		fileStatus: 'addition',
		fileStatusStyle: 'dot',
		clickable: false,
		selected: false,
		conflicted: true,
		checked: true,
		onclick: () => {
			// eslint-disable-next-line no-console
			console.log('clicked');
		},
		onresolveclick: () => {
			// eslint-disable-next-line no-console
			console.log('resolve clicked');
		}
	}}
/>

<Story name="File List">
	{#snippet template()}
		{@const showCheckbox = true}
		<div
			style="width: 500px; display: flex; flex-direction: column; border: 1px solid var(--clr-border-3); border-radius: var(--radius-m); overflow: hidden;"
		>
			<FileListItem
				filePath="src/components/Header.svelte"
				fileStatus="modification"
				fileStatusStyle="dot"
				clickable={true}
				{showCheckbox}
				checked={true}
				draggable={true}
			/>
			<FileListItem
				filePath="src/lib/utils/helper.html"
				fileStatus="addition"
				fileStatusStyle="dot"
				clickable={true}
				{showCheckbox}
				checked={false}
				draggable={true}
			/>
			<FileListItem
				filePath="package.avi"
				fileStatus="modification"
				fileStatusStyle="dot"
				clickable={true}
				{showCheckbox}
				checked={true}
				draggable={true}
			/>
			<FileListItem
				filePath="README.md"
				fileStatus="modification"
				fileStatusStyle="dot"
				clickable={true}
				{showCheckbox}
				checked={true}
				draggable={true}
			/>
			<FileListItem
				filePath="src/styles/main.css"
				fileStatus="deletion"
				fileStatusStyle="dot"
				clickable={true}
				{showCheckbox}
				checked={false}
				draggable={true}
			/>
			<FileListItem
				filePath="config/database.ts"
				fileStatus="addition"
				fileStatusStyle="dot"
				clickable={true}
				{showCheckbox}
				checked={true}
				draggable={true}
			/>
			<FileListItem
				filePath="src/App.svelte"
				fileStatus="modification"
				fileStatusStyle="dot"
				clickable={true}
				selected={true}
				active={true}
				{showCheckbox}
				checked={true}
				draggable={true}
			/>
			<FileListItem
				filePath="tests/unit/helper.test.ts"
				fileStatus="addition"
				fileStatusStyle="dot"
				clickable={true}
				selected={true}
				active={true}
				{showCheckbox}
				checked={true}
				draggable={true}
			/>
			<FileListItem
				filePath="src/routes/index.svelte"
				fileStatus="rename"
				fileStatusStyle="dot"
				clickable={true}
				selected={true}
				active={true}
				{showCheckbox}
				checked={false}
				draggable={true}
			/>
			<FileListItem
				filePath=".gitignore"
				fileStatus="modification"
				fileStatusStyle="dot"
				clickable={true}
				{showCheckbox}
				checked={true}
				draggable={true}
			/>
		</div>
	{/snippet}
</Story>
