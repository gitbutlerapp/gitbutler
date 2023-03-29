<script lang="ts">
	import Tooltip from '$lib/components/Tooltip.svelte';

	export let label: string;
</script>

<!--
@component
This exists only to facilitate testing of the `Tooltip` component in Storybook.
Do not use this in the app.
Do not style this.
Do not add any logic to this.
-->
<div class="bg-[#737373] p-16">
	<Tooltip {label}>
		<span>Hover me!</span>
	</Tooltip>
</div>
