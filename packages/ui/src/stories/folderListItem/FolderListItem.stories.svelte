<script module lang="ts">
	import FolderListItem from '$lib/file/FolderListItem.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'List items / FolderListItem',
		component: FolderListItem,
		args: {
			name: 'Folder',
			showCheckbox: false,
			checked: false,
			isExpanded: true,
			indeterminate: false,
			oncheck: () => {},
			onclick: () => {
				// eslint-disable-next-line no-console
				console.log('Folder clicked');
			},
			onkeydown: () => {},
			ontoggle: (flag) => {
				// eslint-disable-next-line no-console
				console.log('Folder toggled:', flag);
			}
		}
	});
</script>

<Story name="Playground" />
