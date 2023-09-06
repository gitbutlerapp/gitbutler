<script lang="ts">
	import { IconHome } from '../../icons';

	import Button from './Button.svelte';

	export let Hst: import('@histoire/plugin-svelte').Hst;

	let colors = [
		{ label: 'basic', value: 'basic' as const },
		{ label: 'primary', value: 'primary' as const },
		{ label: 'destructive', value: 'destructive' as const },
		{ label: 'purple', value: 'purple' as const }
	];
	let kinds = [
		{ label: 'filled', value: 'filled' as const },
		{ label: 'outlined', value: 'outlined' as const },
		{ label: 'plain', value: 'plain' as const }
	];
	let widths = [
		{ label: 'basic', value: 'basic' as const },
		{ label: 'full-width', value: 'full-width' as const }
	];
	let heights = [
		{ label: 'basic', value: 'basic' as const },
		{ label: 'small', value: 'small' as const }
	];

	let label = 'Button';
	let height = heights[0].value;
	let withIcon = false;
	let loading = false;
	let disabled = false;
	let color = colors[0].value;
	let kind = kinds[0].value;
	let width = widths[0].value;
</script>

<Hst.Story title="Button">
	<Button
		{color}
		{kind}
		icon={withIcon ? IconHome : undefined}
		{loading}
		{width}
		{disabled}
		{height}
		on:click={() => alert('clicked')}
	>
		{#if label}
			{label}
		{/if}
	</Button>

	<svelte:fragment slot="controls">
		<Hst.Text bind:value={label} title="label" />
		<Hst.Checkbox bind:value={withIcon} title="with icon" />
		<Hst.Checkbox bind:value={disabled} title="disabled" />
		<Hst.Checkbox bind:value={loading} title="loading" />
		<Hst.Radio options={heights} bind:value={height} title="height" />
		<Hst.Radio options={colors} bind:value={color} title="color" />
		<Hst.Radio options={kinds} bind:value={kind} title="kind" />
		<Hst.Radio options={widths} bind:value={width} title="width" />
	</svelte:fragment>
</Hst.Story>
