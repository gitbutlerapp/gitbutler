<script module lang="ts">
	import LineStats from '$components/LineStats.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / LineStats',
		component: LineStats,
		args: {
			linesAdded: 42,
			linesRemoved: 13,
			showSeparator: true
		},
		argTypes: {
			linesAdded: {
				control: { type: 'number', min: 0 }
			},
			linesRemoved: {
				control: { type: 'number', min: 0 }
			},
			showSeparator: {
				control: { type: 'boolean' }
			}
		}
	});
</script>

<Story name="Default">
	{#snippet template(args)}
		<LineStats {...args} />
	{/snippet}
</Story>

<Story name="Only Added Lines" args={{ linesAdded: 25, linesRemoved: 0 }}>
	{#snippet template(args)}
		<LineStats {...args} />
	{/snippet}
</Story>

<Story name="Only Removed Lines" args={{ linesAdded: 0, linesRemoved: 18 }}>
	{#snippet template(args)}
		<LineStats {...args} />
	{/snippet}
</Story>

<Story name="Without Separator" args={{ showSeparator: false }}>
	{#snippet template(args)}
		<LineStats {...args} />
	{/snippet}
</Story>

<Story name="Large Numbers" args={{ linesAdded: 1234, linesRemoved: 567 }}>
	{#snippet template(args)}
		<LineStats {...args} />
	{/snippet}
</Story>

<Story name="Zero Changes" args={{ linesAdded: 0, linesRemoved: 0 }}>
	{#snippet template(args)}
		<LineStats {...args} />
	{/snippet}
</Story>

<Story name="In Context" args={{ linesAdded: 15, linesRemoved: 8 }}>
	{#snippet template(args)}
		<div style="display: flex; align-items: center; gap: 8px; font-size: 14px;">
			<span style="font-weight: 600;">Files changed: 3</span>
			<LineStats {...args} />
		</div>
	{/snippet}
</Story>
