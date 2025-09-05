<script module lang="ts">
	import RadioButton from '$components/RadioButton.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / RadioButton',
		component: RadioButton
	});
</script>

<Story name="Playground" />
