<script lang="ts">
	import type { Shade } from '../types/color';

	interface Props {
		shade: Shade;
		color: string;
		scaleId: string;
		onCopy?: (color: string) => void;
		children?: import('svelte').Snippet;
	}

	let { shade, color, scaleId, children }: Props = $props();
</script>

<div
	class="color-card"
	data-scale={scaleId}
	data-shade={shade.value}
	style="background-color: {color}"
	role="presentation"
>
	{#if children}
		{@render children()}
	{/if}
</div>

<style>
	.color-card {
		overflow: hidden;
		--card-ui-color: #fff;
	}
</style>
