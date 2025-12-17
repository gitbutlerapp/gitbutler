<script module lang="ts">
	import { SegmentControl } from '$components/segmentControl';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Segment Control',
		args: {
			selected: '1',
			fullWidth: false,
			size: 'default',
			segments: [
				{ id: '1', icon: 'horz-columns' },
				{ id: '2', icon: 'single-view' },
				{ id: '3', icon: 'vert-columns' }
			]
		},
		argTypes: {
			selected: {
				control: {
					type: 'text'
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
			selected={args.selected}
			fullWidth={args.fullWidth}
			onselect={(id) => {
				// eslint-disable-next-line no-console
				console.log('Selected ID:', id);
			}}
			size={args.size}
		>
			{#each args.segments as segment}
				<SegmentControl.Item id={segment.id} icon={segment.icon}
					>{segment.label}</SegmentControl.Item
				>
			{/each}
		</SegmentControl>
	{/snippet}
</Story>

<Story name="Playground" />
