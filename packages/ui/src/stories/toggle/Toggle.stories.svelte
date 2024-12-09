<script module lang="ts">
	import Toggle from '$lib/Toggle.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Toggle',
		component: Toggle,
		args: {
			small: false,
			disabled: false,
			checked: false,
			value: 'value',
			id: 'id',
			onclick: (e: MouseEvent) => {
				console.log('clicked', e);
			}
		}
	});
</script>

<Story name="Playground" />
