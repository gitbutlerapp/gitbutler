<script module lang="ts">
	import TimeAgo from '$components/TimeAgo.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / Time Ago',
		component: TimeAgo,
		args: {
			date: new Date(),
			addSuffix: true,
			showTooltip: true
		}
	});
</script>

<Story name="Playground" />

<Story name="With Tooltip">
	<div style="padding: 50px; text-align: center;">
		<p>Hover over the time to see absolute timestamp:</p>
		<TimeAgo
			{...{
				date: new Date(Date.now() - 2 * 60 * 60 * 1000), // 2 hours ago
				addSuffix: true,
				showTooltip: true
			}}
		/>
	</div>
</Story>

<Story name="Without Tooltip">
	<div style="padding: 50px; text-align: center;">
		<p>No tooltip shown:</p>
		<TimeAgo
			{...{
				date: new Date(Date.now() - 2 * 60 * 60 * 1000), // 2 hours ago
				addSuffix: true,
				showTooltip: false
			}}
		/>
	</div>
</Story>
