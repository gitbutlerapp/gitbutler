<script module lang="ts">
	import Icon from '$components/Icon.svelte';
	import iconsJson from '$lib/data/icons.json';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / Icon',
		component: Icon,
		args: {
			name: 'ai',
			color: 'pop'
		},
		argTypes: {
			color: {
				control: 'select',
				options: ['gray', 'ghost', 'pop', 'safe', 'error', 'warning', 'purple', undefined]
			},
			name: {
				control: 'select',
				options: Object.keys(iconsJson)
			}
		}
	});
</script>

<Story name="Playground" />
