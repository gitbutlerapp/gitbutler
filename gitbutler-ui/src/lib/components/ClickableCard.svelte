<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let padding: string = 'var(--space-16)';
	export let disabled = false;
	export let checked = false;
	export let hasTopRadius = true;
	export let hasBottomRadius = true;
	export let hasBottomLine = true;

	const SLOTS = $$props.$$slots;

	const dispatchClick = createEventDispatcher<{
		click: void;
	}>();

	const dispatchChange = createEventDispatcher<{
		change: boolean;
	}>();
</script>

<button
	class="clickable-card"
	class:has-top-radius={hasTopRadius}
	class:has-bottom-radius={hasBottomRadius}
	class:has-bottom-line={hasBottomLine}
	style="padding: {padding}"
	on:click={() => {
		dispatchClick('click');

		dispatchChange('change', checked);

		checked = !checked;
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
			<p class="text-base-body-12 clickable-card__text">
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
		border-left: 1px solid var(--clr-theme-container-outline-light);
		border-right: 1px solid var(--clr-theme-container-outline-light);
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

	/* MODIFIERS */

	.has-top-radius {
		border-top: 1px solid var(--clr-theme-container-outline-light);
		border-top-left-radius: var(--radius-l);
		border-top-right-radius: var(--radius-l);
	}

	.has-bottom-radius {
		border-bottom-left-radius: var(--radius-l);
		border-bottom-right-radius: var(--radius-l);
	}

	.has-bottom-line {
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
	}
</style>
