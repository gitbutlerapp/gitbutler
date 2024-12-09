<script module lang="ts">
	import TimeAgo from '$lib/TimeAgo.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / Time Ago',
		component: TimeAgo,
		args: {
			date: new Date(),
			addSuffix: true
		}
	});
</script>

<Story name="Playground" />
