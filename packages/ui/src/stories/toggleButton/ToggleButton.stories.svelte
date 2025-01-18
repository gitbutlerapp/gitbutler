<script module lang="ts">
	import ToggleButton from '$lib/ToggleButton.svelte';
	import iconsJson from '$lib/data/icons.json';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / ToggleButton',
		component: ToggleButton,
		args: {
			disabled: false,
			checked: false,
			label: 'Label',
			icon: 'plus' as keyof typeof iconsJson,
			tooltip: 'Tooltip',
			id: 'id',
			onclick: (e: MouseEvent) => {
				console.log('clicked', e);
			}
		},
		argTypes: {
			icon: {
				options: Object.keys(iconsJson),
				control: {
					type: 'select'
				}
			}
		}
	});
</script>

<Story name="Playground" />
