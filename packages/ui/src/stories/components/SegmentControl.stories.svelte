<script module lang="ts">
	import Segment from '$components/segmentControl/Segment.svelte';
	import SegmentControl from '$components/segmentControl/SegmentControl.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Segment Control',
		args: {
			defaultIndex: 0,
			fullWidth: false,
			size: 'default'
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

<Story name="Icons Only">
	{#snippet template(args)}
		<SegmentControl
			defaultIndex={args.defaultIndex}
			fullWidth={args.fullWidth}
			onselect={(id) => {
				// eslint-disable-next-line no-console
				console.log('Selected segment ID:', id);
			}}
			size={args.size}
		>
			<Segment id="horizontal" icon="horz-columns" />
			<Segment id="single" icon="single-view" />
			<Segment id="vertical" icon="vert-columns" />
			<Segment id="list" icon="list-view" />
		</SegmentControl>
	{/snippet}
</Story>

<Story name="Labels with Icons">
	{#snippet template(args)}
		<SegmentControl
			defaultIndex={args.defaultIndex}
			fullWidth={args.fullWidth}
			onselect={(id) => {
				// eslint-disable-next-line no-console
				console.log('Selected segment ID:', id);
			}}
			size={args.size}
		>
			<Segment id="files" icon="file-changes">Files</Segment>
			<Segment id="commits" icon="commit">Commits</Segment>
			<Segment id="branches" icon="branches">Branches</Segment>
			<Segment id="settings" icon="settings">Settings</Segment>
		</SegmentControl>
	{/snippet}
</Story>

<Story name="Labels Only">
	{#snippet template(args)}
		<SegmentControl
			defaultIndex={args.defaultIndex}
			fullWidth={args.fullWidth}
			onselect={(id) => {
				// eslint-disable-next-line no-console
				console.log('Selected segment ID:', id);
			}}
			size={args.size}
		>
			<Segment id="overview">Overview</Segment>
			<Segment id="details">Details</Segment>
			<Segment id="history">History</Segment>
			<Segment id="analytics">Analytics</Segment>
		</SegmentControl>
	{/snippet}
</Story>

<Story name="Small Size">
	{#snippet template(_args)}
		<SegmentControl
			defaultIndex={0}
			size="small"
			onselect={(id) => {
				// eslint-disable-next-line no-console
				console.log('Selected segment ID:', id);
			}}
		>
			<Segment id="view1" icon="horz-columns">View 1</Segment>
			<Segment id="view2" icon="single-view">View 2</Segment>
			<Segment id="view3" icon="vert-columns">View 3</Segment>
		</SegmentControl>
	{/snippet}
</Story>

<Story name="Full Width">
	{#snippet template(_args)}
		<SegmentControl
			defaultIndex={0}
			fullWidth={true}
			onselect={(id) => {
				// eslint-disable-next-line no-console
				console.log('Selected segment ID:', id);
			}}
		>
			<Segment id="tab1">First Tab</Segment>
			<Segment id="tab2">Second Tab</Segment>
			<Segment id="tab3">Third Tab</Segment>
			<Segment id="tab4">Fourth Tab</Segment>
		</SegmentControl>
	{/snippet}
</Story>
