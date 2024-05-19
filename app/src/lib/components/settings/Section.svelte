<script lang="ts">
	import Spacer from '../Spacer.svelte';
	export let spacer = false;
	export let gap = 'var(--size-20)';

	const SLOTS = $$props.$$slots;
</script>

<div class="settings-section" style="gap: {gap}">
	{#if SLOTS.top}
		<slot name="top" />
	{/if}

	{#if SLOTS.title || SLOTS.description}
		<div class="description">
			{#if SLOTS.title}
				<h2 class="text-base-15 text-bold">
					<slot name="title" />
				</h2>
			{/if}
			{#if SLOTS.description}
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
		gap: var(--size-10);
	}

	.description h2 {
		color: var(--clr-text-1);
	}

	.description p {
		color: var(--clr-text-2);
	}
</style>
