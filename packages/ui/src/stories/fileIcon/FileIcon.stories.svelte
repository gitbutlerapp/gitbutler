<script module lang="ts">
	import FileIcon from '$lib/file/FileIcon.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / FileIcon',
		component: FileIcon,
		args: {
			fileName: 'file.txt',
			size: 16
		},
		argTypes: {}
	});
</script>

<Story name="Default" />
