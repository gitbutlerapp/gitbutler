<script module lang="ts">
	import Segment from '$lib/segmentControl/Segment.svelte';
	import SegmentControl from '$lib/segmentControl/SegmentControl.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Segment Controld',
		args: {
			defaultIndex: 0,
			fullWidth: false,
			segments: [
				{ id: '1', label: 'Segment 1' },
				{ id: '2', label: 'Segment 2' },
				{ id: '3', label: 'Segment 3' }
			]
		},
		argTypes: {
			defaultIndex: {
				control: {
					type: 'number'
				}
			}
		}
	});
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<SegmentControl
		defaultIndex={args.defaultIndex}
		fullWidth={args.fullWidth}
		onselect={(id) => {
			console.log('Selected index:', id);
		}}
	>
		{#each args.segments as segment}
			<Segment id={segment.id}>{segment.label}</Segment>
		{/each}
	</SegmentControl>
{/snippet}

<Story name="Playground" />
