<script module lang="ts">
	import Spacer from '$lib/Spacer.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / Spacer',
		component: Spacer,
		args: {
			margin: 0,
			noLine: false,
			dotted: false
		}
	});
</script>

<Story name="Default" />
