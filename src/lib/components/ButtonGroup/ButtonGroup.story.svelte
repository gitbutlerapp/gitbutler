<script lang="ts">
	import ButtonGroup from './ButtonGroup.svelte';

	export let Hst: import('@histoire/plugin-svelte').Hst;

	const noop = () => {};

	let widths = [
		{ label: 'basic', value: 'basic' as const },
		{ label: 'full-width', value: 'full-width' as const }
	];

	let width = widths[0].value;
</script>

<Hst.Story title="Button Group">
	<svelte:fragment slot="controls">
		<Hst.Radio options={widths} bind:value={width} title="width" />
	</svelte:fragment>

	<Hst.Variant title="Two buttons">
		<ButtonGroup
			{width}
			leftLabel="Cancel"
			rightLabel="Submit"
			leftAction={noop}
			rightAction={noop}
		/>
	</Hst.Variant>

	<Hst.Variant title="Three buttons">
		<ButtonGroup
			{width}
			leftLabel="Cancel"
			rightLabel="Submit"
			middleLabel="Middle"
			leftAction={noop}
			rightAction={noop}
		/>
	</Hst.Variant>
</Hst.Story>
