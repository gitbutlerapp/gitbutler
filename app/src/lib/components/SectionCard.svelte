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
	export let clickable = false;

	const dispatch = createEventDispatcher<{ hover: boolean }>();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
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
	class:loading={background === 'loading'}
	class:success={background === 'success'}
	class:error={background === 'error'}
	class:clickable={labelFor !== '' || clickable}
	class:disabled
	on:click
	on:mouseenter={() => dispatch('hover', true)}
	on:mouseleave={() => dispatch('hover', false)}
>
	{#if $$slots.iconSide}
		<div class="section-card__icon-side">
			<slot name="iconSide" />
		</div>
	{/if}

	{#if $$slots.title || $$slots.caption}
		<div class="section-card__content">
			{#if $$slots.title}
				<h3 class="text-base-15 text-bold section-card__title">
					<slot name="title" />
				</h3>
			{/if}
			{#if $$slots.caption}
				<p class="text-base-body-12 section-card__text">
					<slot name="caption" />
				</p>
			{/if}
		</div>
	{/if}

	<slot />

	{#if $$slots.actions}
		<div class="section-card__actions">
			<slot name="actions" />
		</div>
	{/if}
</label>

<style lang="postcss">
	.section-card {
		position: relative;
		display: flex;
		gap: 16px;
		padding: 16px;
		border-left-width: 1px;
		border-right-width: 1px;
		border-color: var(--clr-border-2);
		background-color: var(--clr-bg-1);
		cursor: default;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);
		text-align: left;
	}

	.loading {
		background: var(--clr-bg-2);
	}

	.success {
		background: var(--clr-theme-pop-bg);
	}

	.error {
		background: var(--clr-theme-warn-bg);
	}
	.extra-padding {
		padding: 20px;
	}

	.section-card__content {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 8px;
		user-select: text;
	}

	.section-card__title {
		color: var(--clr-scale-ntrl-0);
	}

	.section-card__text {
		color: var(--clr-text-2);

		/* if empty hide the caption */
		&:empty {
			display: none;
		}
	}

	.section-card__actions {
		display: flex;
	}

	.section-card__icon-side {
		display: flex;
		align-items: center;
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
			background-color: var(--clr-border-3);
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
