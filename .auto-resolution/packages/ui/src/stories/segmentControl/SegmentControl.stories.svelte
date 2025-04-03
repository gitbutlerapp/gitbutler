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
			size: 'default',
			segments: [
				{ id: '1', icon: 'tree-view' },
				{ id: '2', icon: 'tree-view' },
				{ id: '3', icon: 'tree-view' }
			]
		},
		argTypes: {
			defaultIndex: {
				control: {
					type: 'number'
				}
			},
			fullWidth: {
				control: {
					type: 'boolean'
				}
			},
			size: {
				options: ['default', 'small'],
				control: {
					type: 'select'
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
			// eslint-disable-next-line no-console
			console.log('Selected index:', id);
		}}
		size={args.size}
	>
		{#each args.segments as segment}
			<Segment id={segment.id} icon={segment.icon}>{segment.label}</Segment>
		{/each}
	</SegmentControl>
{/snippet}

<Story name="Playground" />
