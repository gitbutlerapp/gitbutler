<script lang="ts">
	import Button from '$lib/components/Button.svelte';

	export let leftLabel: string;
	export let leftAction: () => void;
	export let rightLabel: string;
	export let rightAction: () => void;
	export let middleLabel: string | undefined;
	export let middleAction: () => void | undefined;
	export let wide = false;
</script>

{#if !middleLabel}
	<div class="btn-group">
		<Button label={leftLabel} on:click={leftAction} outlined={true} {wide} />
		<Button label={rightLabel} on:click={rightAction} primary={true} outlined={false} {wide} />
	</div>
{:else}
	<div class="btn-group btn-group--segmented">
		<button class="joined-base rounded-l-lg border-l border-t border-b" on:click={leftAction}>
			<span class="my-2 {wide ? 'mx-[31.5px]' : 'mx-[16px]'}">{leftLabel}</span>
		</button>
		<button class="joined-base border" on:click={middleAction}>
			<span class="my-2 {wide ? 'mx-[31.5px]' : 'mx-[16px]'}">{middleLabel}</span>
		</button>
		<button class="joined-base rounded-r-lg border-r border-t border-b" on:click={rightAction}>
			<span class="my-2 {wide ? 'mx-[31.5px]' : 'mx-[16px]'}">{rightLabel}</span>
		</button>
	</div>
{/if}

<style>
	.joined-base {
		@apply flex items-center justify-center border-[#71717a] text-base text-white;
	}
	.btn-group {
		@apply flex gap-3;
	}
	.btn-group--segmented {
		@apply flex gap-0;
	}
</style>
