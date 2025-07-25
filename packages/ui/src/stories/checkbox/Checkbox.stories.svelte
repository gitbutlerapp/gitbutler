<script module lang="ts">
	import Checkbox from '$components/Checkbox.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Checkbox',
		component: Checkbox,
		args: {
			checked: false,
			disabled: false,
			indeterminate: false,
			small: false
		}
	});
</script>

<Story name="Playground" />
