<script lang="ts" context="module">
	export type SectionCardBackground = 'loading' | 'success' | 'error' | undefined;
</script>

<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let orientation: 'row' | 'column' = 'column';
	export let extraPadding = false;
	export let roundedTop = true;
	export let roundedBottom = true;
	export let bottomBorder = true;
	export let background: SectionCardBackground = undefined;
	export let noBorder = false;
	export let labelFor = '';
	export let disabled = false;

	const SLOTS = $$props.$$slots;

	const dispatch = createEventDispatcher<{ hover: boolean }>();
</script>

<label
	for={labelFor}
	class="section-card"
	style:flex-direction={orientation}
	class:extra-padding={extraPadding}
	class:rounded-top={roundedTop}
	class:rounded-bottom={roundedBottom}
	class:bottom-border={bottomBorder}
	class:no-border={noBorder}
	class:loading={background == 'loading'}
	class:success={background == 'success'}
	class:error={background == 'error'}
	class:clickable={labelFor !== ''}
	class:disabled
	on:mouseenter={() => dispatch('hover', true)}
	on:mouseleave={() => dispatch('hover', false)}
>
	{#if SLOTS.iconSide}
		<div class="section-card__icon-side">
			<slot name="iconSide" />
		</div>
	{/if}

	{#if SLOTS.title || SLOTS.caption}
		<div class="section-card__content">
			{#if SLOTS.title}
				<h3 class="text-base-15 text-bold section-card__title">
					<slot name="title" />
				</h3>
			{/if}
			{#if SLOTS.caption}
				<p class="text-base-body-12 section-card__text">
					<slot name="caption" />
				</p>
			{/if}
		</div>
	{/if}

	<slot />

	{#if SLOTS.actions}
		<div class="section-card__actions">
			<slot name="actions" />
		</div>
	{/if}
</label>

<style lang="post-css">
	.section-card {
		display: flex;
		gap: var(--size-16);
		padding: var(--size-16);
		border-left: 1px solid var(--clr-theme-container-outline-light);
		border-right: 1px solid var(--clr-theme-container-outline-light);
		background-color: var(--clr-theme-container-light);
		cursor: default;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);
		text-align: left;
	}

	.loading {
		background: var(--clr-theme-container-pale);
	}

	.success {
		background: var(--clr-theme-pop-container);
	}

	.error {
		background: var(--clr-theme-warn-container);
	}
	.extra-padding {
		padding: var(--size-20);
	}

	.section-card__content {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
		user-select: text;
	}

	.section-card__title {
		color: var(--clr-theme-scale-ntrl-0);
	}

	.section-card__text {
		color: var(--clr-theme-scale-ntrl-30);
	}

	/* MODIFIERS */

	.rounded-top {
		border-top: 1px solid var(--clr-theme-container-outline-light);
		border-top-left-radius: var(--radius-m);
		border-top-right-radius: var(--radius-m);
	}

	.rounded-bottom {
		border-bottom-left-radius: var(--radius-m);
		border-bottom-right-radius: var(--radius-m);
	}

	.bottom-border {
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
	}

	.no-border {
		border: none;
	}

	.clickable {
		cursor: pointer;

		&:hover {
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-light),
				var(--darken-tint-extralight)
			);
		}
	}

	.disabled {
		pointer-events: none;
		opacity: 0.5;
	}
</style>
