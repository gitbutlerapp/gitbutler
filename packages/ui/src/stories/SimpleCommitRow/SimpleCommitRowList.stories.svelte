<script module lang="ts">
	import SimpleCommitRow, { type Props as SeriesProps } from '$components/SimpleCommitRow.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const items = [
		{
			title: 'feat: add user authentication',
			sha: 'b52f23',
			author: 'Jane Smith',
			date: new Date('2022-01-15T08:30:00Z')
		},
		{
			title: 'refactor: improve database queries',
			sha: 'a9f732',
			author: 'Mike Brown',
			date: new Date('2023-05-23T14:15:00Z')
		},
		{
			title: 'fix: correct null pointer exception',
			sha: 'e7c245',
			author: 'Alice White',
			date: new Date('2023-07-12T09:45:00Z')
		},
		{
			title: 'docs: update README with API examples',
			sha: '2ac923',
			author: 'Sam Green',
			date: new Date('2021-10-21T17:00:00Z')
		}
	] as SeriesProps[];

	const { Story } = defineMeta({
		title: 'List items / Simple Commit Row'
	});
</script>

<Story name="Variants">
	<div class="wrapper">
		{#each items as item}
			<SimpleCommitRow {...item} />
		{/each}
	</div>
</Story>

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
</style>
