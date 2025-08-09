<script module lang="ts">
	import Switch from '$components/Switch.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Switch',
		component: Switch,
		args: {
			checked: false
		}
	});
</script>

<Story name="Playground" />
