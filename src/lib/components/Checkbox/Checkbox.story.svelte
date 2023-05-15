<script lang="ts">
	import Checkbox from './Checkbox.svelte';

	export let Hst: import('@histoire/plugin-svelte').Hst;

	let large = false;
	let disabled = false;
	let error = false;
	let checked = false;
	let indeterminate = false;
</script>

<Hst.Story title="Checkbox">
	<svelte:fragment slot="controls">
		<Hst.Checkbox bind:value={large} title="large" />
		<Hst.Checkbox bind:value={disabled} title="disabled" />
		<Hst.Checkbox bind:value={error} title="error" />
		<Hst.Checkbox bind:value={checked} title="checked" />
		<Hst.Checkbox bind:value={indeterminate} title="indeterminate" />
	</svelte:fragment>

	<Checkbox {large} {disabled} {error} bind:checked bind:indeterminate />
</Hst.Story>
