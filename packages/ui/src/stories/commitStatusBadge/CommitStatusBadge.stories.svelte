<script module lang="ts">
	import CommitStatusBadge from '$lib/CommitStatusBadge.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'ButReview / CommitStatusBadge',
		component: CommitStatusBadge
	});
</script>

<Story name="Playground" />
