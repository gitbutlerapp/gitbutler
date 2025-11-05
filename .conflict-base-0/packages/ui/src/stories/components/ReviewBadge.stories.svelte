<script module lang="ts">
	import ReviewBadge from '$components/ReviewBadge.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / Review Badge',
		component: ReviewBadge,
		args: {
			type: 'PR',
			status: 'open',
			number: 1
		}
	});
</script>

<script lang="ts">
</script>

<Story name="default">
	{#snippet template(args)}
		<ReviewBadge type={args.type} status={args.status} number={args.number} title={args.title} />
	{/snippet}
</Story>

<Story name="Playground" />
