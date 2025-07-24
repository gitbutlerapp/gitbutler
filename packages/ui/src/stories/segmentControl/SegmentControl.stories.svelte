<script module lang="ts">
	import Segment from '$lib/segmentControl/Segment.svelte';
	import SegmentControl from '$lib/segmentControl/SegmentControl.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Segment Control',
		args: {
			defaultIndex: 0,
			fullWidth: false,
			size: 'default',
			segments: [
				{ id: '1', icon: 'horz-columns' },
				{ id: '2', icon: 'single-view' },
				{ id: '3', icon: 'vert-columns' }
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
</script>

<Story name="default">
	{#snippet template(args)}
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
</Story>

<Story name="Playground" />
