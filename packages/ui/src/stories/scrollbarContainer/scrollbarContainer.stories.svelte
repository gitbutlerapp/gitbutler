<script module lang="ts">
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';

	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / ScrollableContainer',
		component: ScrollableContainer,
		args: {
			whenToShow: 'always'
		},
		argTypes: {
			whenToShow: {
				options: ['always', 'hover', 'scroll'],
				control: { type: 'select' }
			}
		}
	});
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<div class="list-wrapper">
		<ScrollableContainer whenToShow={args.whenToShow ?? 'always'}>
			{#each Array(50) as _, i}
				<div class="item">Item {i}</div>
			{/each}

			<div class="list-wrapper"></div>
		</ScrollableContainer>
	</div>
{/snippet}

<Story name="Playground" />

<style>
	.list-wrapper {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		border: 1px solid var(--clr-border-2);
		max-height: 300px;
		overflow: hidden;
	}

	.item {
		background-color: var(--clr-bg-1);
		padding: 4px;
		border: 1px solid var(--clr-border-2);
	}
</style>
