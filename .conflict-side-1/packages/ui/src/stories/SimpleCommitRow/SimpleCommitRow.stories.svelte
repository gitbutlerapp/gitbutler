<script module lang="ts">
	import SimpleCommitRow from '$lib/SimpleCommitRow.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'List items / Simple Commit Row',
		component: SimpleCommitRow,
		args: {
			title: 'feat: add user authentication',
			sha: 'b52f23',
			author: 'Jane Smith',
			date: new Date('2022-01-15T08:30:00Z')
		}
	});
</script>

<Story name="Playground" />
