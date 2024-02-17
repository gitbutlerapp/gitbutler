<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let padding: string = 'var(--space-16)';
	export let disabled = false;

	const SLOTS = $$props.$$slots;

	const dispatch = createEventDispatcher<{
		click: void;
	}>();
</script>

<button
	class="clickable-card"
	style="padding: {padding}"
	on:click={() => {
		dispatch('click');
	}}
	class:card-disabled={disabled}
	{disabled}
>
	<div class="clickable-card__content">
		{#if SLOTS.title}
			<h3 class="text-base-15 text-bold clickable-card__title">
				<slot name="title" />
			</h3>
		{/if}
		{#if SLOTS.body}
			<p class="text-base-12 clickable-card__text">
				<slot name="body" />
			</p>
		{/if}
	</div>
	{#if SLOTS.actions}
		<div class="clickable-card__actions">
			<slot name="actions" />
		</div>
	{/if}
</button>

<style lang="post-css">
	.clickable-card {
		display: flex;
		gap: var(--space-16);
		border-radius: var(--radius-l);
		border: 1px solid var(--clr-theme-container-outline-light);
		background-color: var(--clr-theme-container-light);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);
		text-align: left;

		&:hover {
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-light),
				var(--darken-tint-extralight)
			);
		}
	}

	.card-disabled {
		opacity: 0.6;
		pointer-events: none;
	}

	.clickable-card__content {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: var(--space-8);
	}

	.clickable-card__title {
		color: var(--clr-theme-scale-ntrl-0);
	}

	.clickable-card__text {
		color: var(--clr-theme-scale-ntrl-30);
	}

	.clickable-card__actions {
		display: flex;
		flex-shrink: 0;
	}
</style>
