<script lang="ts">
	import Tooltip from './Tooltip.svelte';

	export let Hst: import('@histoire/plugin-svelte').Hst;

	let label = 'This is a tooltip';
</script>

<Hst.Story title="Tooltip">
	<Tooltip {label}>
		<span class="text-zinc-50">Hover me!</span>
	</Tooltip>

	<svelte:fragment slot="controls">
		<Hst.Text bind:value={label} title="label" />
	</svelte:fragment>
</Hst.Story>
