<script module lang="ts">
	import Badge from '$lib/Badge.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / Badge',
		component: Badge,
		args: {
			label: 127,
			style: 'neutral',
			kind: 'solid'
		},
		argTypes: {
			style: {
				options: ['neutral', 'success', 'warning', 'error'],
				control: { type: 'select' }
			},
			kind: {
				options: ['solid', 'soft'],
				control: { type: 'select' }
			}
		}
	});
</script>

<Story name="Playground" />
