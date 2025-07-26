<script module lang="ts">
	import ScrollableContainer from '$components/scroll/ScrollableContainer.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / ScrollableContainer',
		component: ScrollableContainer,
		args: {
			whenToShow: 'always',
			children
		},
		argTypes: {
			whenToShow: {
				options: ['always', 'hover', 'scroll'],
				control: { type: 'select' }
			}
		}
	});
</script>

{#snippet children()}
	{#each Array(50) as _, i}
		<div class="item">Item {i}</div>
	{/each}

	<div class="list-wrapper"></div>
{/snippet}

<Story name="default">
	{#snippet template(args)}
		<div class="list-wrapper">
			<ScrollableContainer whenToShow={args.whenToShow ?? 'always'} children={args.children} />
		</div>
	{/snippet}
</Story>

<Story name="Playground" />

<style>
	.list-wrapper {
		display: flex;
		flex-direction: column;
		max-height: 300px;
		overflow: hidden;
		gap: 0.5rem;
		border: 1px solid var(--clr-border-2);
	}

	.item {
		padding: 4px;
		border: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}
</style>
