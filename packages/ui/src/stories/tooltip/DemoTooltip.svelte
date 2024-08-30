<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';

	const props = $props();
</script>

<div class="wrapper">
	<p class="text-13 text">
		hello world! Here is a <Tooltip text={props.text}>
			<span class="tooltip-text">tooltip</span>
		</Tooltip> for you.
	</p>
</div>

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		padding: 20px;
	}

	.text {
		color: var(--clr-text-1);
	}

	.tooltip-text {
		text-decoration: underline;
		text-decoration-style: dotted;
	}
</style>
