<script module lang="ts">
	import ReviewBadge from '$lib/ReviewBadge.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / Review Badge',
		component: ReviewBadge,
		args: {
			type: 'PR',
			status: 'open',
			number: 1,
			title: 'Sample Pull Request'
		}
	});
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template(
	{ type = 'PR', ...rest }: Args<typeof Story>,
	_context: StoryContext<typeof Story>
)}
	<ReviewBadge {type} {...rest} />
{/snippet}

<Story name="Playground" />
