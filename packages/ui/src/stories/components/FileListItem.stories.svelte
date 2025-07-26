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
		fileStatus: 'A',
		fileStatusStyle: 'dot',
		clickable: true,
		selected: false,
		conflicted: true,
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
		fileStatus: 'A',
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
