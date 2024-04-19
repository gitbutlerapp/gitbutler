<script lang="ts" context="module">
	export type SectionCardBackground = 'loading' | 'success' | 'error' | undefined;
</script>

<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let orientation: 'row' | 'column' = 'column';
	export let centerAlign = false;
	export let extraPadding = false;
	export let roundedTop = true;
	export let roundedBottom = true;
	export let topDivider = false;
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
	class:center-align={centerAlign && orientation === 'row'}
	class:extra-padding={extraPadding}
	class:rounded-top={roundedTop}
	class:rounded-bottom={roundedBottom}
	class:top-divider={topDivider}
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
		position: relative;
		display: flex;
		gap: var(--size-16);
		padding: var(--size-16);
		border-left-width: 1px;
		border-right-width: 1px;
		border-color: var(--clr-border-main);
		background-color: var(--clr-bg-main);
		cursor: default;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);
		text-align: left;
	}

	.loading {
		background: var(--clr-bg-alt);
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
		color: var(--clr-scale-ntrl-0);
	}

	.section-card__text {
		color: var(--clr-scale-ntrl-30);

		/* if empty hide the caption */
		&:empty {
			display: none;
		}
	}

	.section-card__actions {
		display: flex;
	}

	/* MODIFIERS */

	.rounded-top {
		border-top-width: 1px;
		border-top-left-radius: var(--radius-m);
		border-top-right-radius: var(--radius-m);
	}

	.rounded-bottom {
		border-bottom-left-radius: var(--radius-m);
		border-bottom-right-radius: var(--radius-m);
	}

	.top-divider {
		&::before {
			content: '';
			position: absolute;
			top: 0;
			left: 0;
			display: block;
			width: 100%;
			height: 1px;
			background-color: var(--clr-border-main);
			opacity: 0.5;
		}
	}

	.bottom-border {
		border-bottom-width: 1px;
	}

	.no-border {
		border-width: none;
	}

	.clickable {
		cursor: pointer;
	}

	.disabled {
		pointer-events: none;
		opacity: 0.5;
	}

	.center-align {
		align-items: center;
	}
</style>
