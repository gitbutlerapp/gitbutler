<script module lang="ts">
	import Checkbox from '$lib/Checkbox.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Checkbox',
		component: Checkbox,
		args: {
			style: 'default',
			checked: false,
			disabled: false,
			indeterminate: false,
			small: false
		}
	});
</script>

<Story name="Playground" />
