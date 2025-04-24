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
			prStatus: 'open',
			prNumber: 1
		},
		argTypes: {
			prStatus: {
				options: ['open', 'closed', 'draft', 'merged', 'unknown', undefined],
				control: { type: 'select' }
			},
			prNumber: {
				control: { type: 'number' }
			},
			brStatus: {
				options: [
					'approved',
					'unreviewed',
					'changes_requested',
					'in-discussion',
					'unknown',
					undefined
				],
				control: { type: 'select' }
			},
			brId: {
				control: { type: 'number' }
			}
		}
	});
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<ReviewBadge {...args} />
{/snippet}

<Story name="Playground" />
