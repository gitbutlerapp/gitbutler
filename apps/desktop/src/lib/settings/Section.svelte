<script lang="ts">
	import Spacer from '$lib/shared/Spacer.svelte';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';

	export let spacer = false;
	export let gap = 16;
</script>

<div class="settings-section" style="gap: {pxToRem(gap)}">
	{#if $$slots.top}
		<slot name="top" />
	{/if}

	{#if $$slots.title || $$slots.description}
		<div class="description">
			{#if $$slots.title}
				<h2 class="text-15 text-bold">
					<slot name="title" />
				</h2>
			{/if}
			{#if $$slots.description}
				<p class="text-12 text-body">
					<slot name="description" />
				</p>
			{/if}
		</div>
	{/if}

	<slot />

	{#if spacer}
		<Spacer />
	{/if}
</div>

<style>
	.settings-section {
		display: flex;
		flex-direction: column;
	}

	.description {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.description h2 {
		color: var(--clr-text-1);
	}

	.description p {
		color: var(--clr-text-2);
	}
</style>
