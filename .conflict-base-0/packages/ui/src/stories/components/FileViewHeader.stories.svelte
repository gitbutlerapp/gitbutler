<script module lang="ts">
	import FileViewHeader from '$components/file/FileViewHeader.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'List items / FileViewHeader',
		component: FileViewHeader
	});
</script>

<Story
	name="Playground"
	args={{
		filePath: 'src/component/arcada/path/binary/file.svelte',
		fileStatus: 'modification',
		draggable: true,
		linesAdded: 10,
		linesRemoved: 5,
		oncloseclick: () => {
			alert('close clicked');
		}
	}}
/>
