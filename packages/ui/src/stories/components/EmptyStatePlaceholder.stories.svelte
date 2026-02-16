<script module lang="ts">
	import Button from '$components/Button.svelte';
	import EmptyStatePlaceholder from '$components/EmptyStatePlaceholder.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Layout / EmptyStatePlaceholder',
		component: EmptyStatePlaceholder,
		args: {
			width: 256,
			bottomMargin: 0,
			topBottomPadding: 48,
			leftRightPadding: 0,
			gap: 16,
			background: 'none'
		},
		argTypes: {
			width: {
				control: { type: 'number' }
			},
			bottomMargin: {
				control: { type: 'number' }
			},
			topBottomPadding: {
				control: { type: 'number' }
			},
			leftRightPadding: {
				control: { type: 'number' }
			},
			gap: {
				control: { type: 'number' }
			},
			background: {
				control: { type: 'text' }
			}
		}
	});
</script>

<Story name="default">
	{#snippet template(args)}
		<EmptyStatePlaceholder
			width={args.width}
			bottomMargin={args.bottomMargin}
			topBottomPadding={args.topBottomPadding}
			leftRightPadding={args.leftRightPadding}
			gap={args.gap}
			background={args.background}
		>
			{#snippet title()}
				No items found
			{/snippet}
			{#snippet caption()}
				There are no items to display at the moment.
			{/snippet}
		</EmptyStatePlaceholder>
	{/snippet}
</Story>

<Story name="With Actions">
	{#snippet template(args)}
		<EmptyStatePlaceholder
			width={args.width}
			bottomMargin={args.bottomMargin}
			topBottomPadding={args.topBottomPadding}
			leftRightPadding={args.leftRightPadding}
			gap={args.gap}
			background={args.background}
		>
			{#snippet title()}
				No branches yet
			{/snippet}
			{#snippet caption()}
				Create your first branch to get started with your project.
			{/snippet}
			{#snippet actions()}
				<Button style="pop" onclick={() => alert('Create branch clicked')}>Create Branch</Button>
			{/snippet}
		</EmptyStatePlaceholder>
	{/snippet}
</Story>

<Story name="With Image">
	{#snippet template(args)}
		<EmptyStatePlaceholder
			width={args.width}
			bottomMargin={args.bottomMargin}
			topBottomPadding={args.topBottomPadding}
			leftRightPadding={args.leftRightPadding}
			gap={args.gap}
			background={args.background}
			image="<svg width=&quot;80&quot; height=&quot;80&quot; viewBox=&quot;0 0 80 80&quot; fill=&quot;none&quot; xmlns=&quot;http://www.w3.org/2000/svg&quot;><circle cx=&quot;40&quot; cy=&quot;40&quot; r=&quot;40&quot; fill=&quot;#E5E7EB&quot;/><path d=&quot;M40 20v40M20 40h40&quot; stroke=&quot;#9CA3AF&quot; stroke-width=&quot;4&quot; stroke-linecap=&quot;round&quot;/></svg>"
		>
			{#snippet title()}
				Nothing here yet
			{/snippet}
			{#snippet caption()}
				Add something to get started.
			{/snippet}
		</EmptyStatePlaceholder>
	{/snippet}
</Story>

<Story name="With Background">
	{#snippet template()}
		<EmptyStatePlaceholder
			width={320}
			topBottomPadding={64}
			leftRightPadding={24}
			background="var(--clr-bg-2)"
		>
			{#snippet title()}
				Empty workspace
			{/snippet}
			{#snippet caption()}
				Your workspace is empty. Start by adding some files or creating a new project.
			{/snippet}
			{#snippet actions()}
				<Button style="gray">Browse Files</Button>
			{/snippet}
		</EmptyStatePlaceholder>
	{/snippet}
</Story>

<Story name="Playground" />
