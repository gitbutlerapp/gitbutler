<script lang="ts">
	import Spacer from '$lib/shared/Spacer.svelte';
	import { pxToRem } from '$lib/utils/pxToRem';

	export let spacer = false;
	export let gap = 20;
</script>

<div class="settings-section" style="gap: {pxToRem(gap)}">
	{#if $$slots.top}
		<slot name="top" />
	{/if}

	{#if $$slots.title || $$slots.description}
		<div class="description">
			{#if $$slots.title}
				<h2 class="text-base-15 text-bold">
					<slot name="title" />
				</h2>
			{/if}
			{#if $$slots.description}
				<p class="text-base-body-12">
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
