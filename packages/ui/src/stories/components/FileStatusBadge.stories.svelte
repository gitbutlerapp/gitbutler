<script module lang="ts">
	import FileStatusBadge from '$components/file/FileStatusBadge.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / FileStatusBadge',
		component: FileStatusBadge,
		args: {
			status: 'modification',
			style: 'full',
			tooltip: undefined
		},
		argTypes: {
			status: {
				options: ['addition', 'modification', 'deletion', 'rename'],
				control: { type: 'select' }
			},
			style: {
				options: ['dot', 'full', 'full-large'],
				control: { type: 'select' }
			},
			tooltip: {
				control: { type: 'text' }
			}
		}
	});
</script>

<Story name="Playground" />

<Story name="All Statuses">
	{#snippet template()}
		<div style="display: flex; gap: 12px; align-items: center;">
			<FileStatusBadge status="addition" style="full" />
			<FileStatusBadge status="modification" style="full" />
			<FileStatusBadge status="deletion" style="full" />
			<FileStatusBadge status="rename" style="full" tooltip="oldname.txt → newname.txt" />
		</div>
	{/snippet}
</Story>

<Story name="Dot Style">
	{#snippet template()}
		<div style="display: flex; gap: 12px; align-items: center;">
			<FileStatusBadge status="addition" style="dot" />
			<FileStatusBadge status="modification" style="dot" />
			<FileStatusBadge status="deletion" style="dot" />
			<FileStatusBadge status="rename" style="dot" />
		</div>
	{/snippet}
</Story>

<Story name="Full Large">
	{#snippet template()}
		<div style="display: flex; gap: 12px; align-items: center;">
			<FileStatusBadge status="addition" style="full-large" />
			<FileStatusBadge status="modification" style="full-large" />
			<FileStatusBadge status="deletion" style="full-large" />
			<FileStatusBadge status="rename" style="full-large" tooltip="oldname.txt → newname.txt" />
		</div>
	{/snippet}
</Story>
