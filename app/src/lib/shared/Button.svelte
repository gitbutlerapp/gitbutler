<script lang="ts">
	import Icon from '$lib/shared/Icon.svelte';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { tooltip } from '$lib/utils/tooltip';
	import type iconsJson from '$lib/icons/icons.json';
	import type { ComponentColor, ComponentStyleKind } from '$lib/vbranches/types';

	// Interaction props
	export let element: HTMLAnchorElement | HTMLButtonElement | HTMLElement | null = null;
	export let disabled = false;
	export let clickable = true;
	export let id: string | undefined = undefined;
	export let loading = false;
	export let tabindex: number | undefined = undefined;
	export let type: 'submit' | 'reset' | 'button' | undefined = undefined;
	// Layout props
	export let shrinkable = false;
	export let reversedDirection: boolean = false;
	export let width: number | undefined = undefined;
	export let size: 'tag' | 'button' | 'cta' = 'button';
	export let wide = false;
	export let grow = false;
	export let align: 'flex-start' | 'center' | 'flex-end' | 'stretch' | 'baseline' | 'auto' = 'auto';
	export let isDropdownChild = false;
	// Style props
	export let style: ComponentColor = 'neutral';
	export let kind: ComponentStyleKind = 'soft';
	export let outline = false;
	export let dashed = false;
	export let solidBackground = false;
	// Additional elements
	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let help = '';
	export let helpShowDelay = 1200;
</script>

<button
	class="btn focus-state {style} {kind} {size}-size"
	class:outline
	class:dashed
	class:solidBackground
	class:reversed-direction={reversedDirection}
	class:shrinkable
	class:wide
	class:grow
	class:not-clickable={!clickable}
	class:fixed-width={!$$slots.default && !wide}
	class:is-dropdown={isDropdownChild}
	style:align-self={align}
	style:width={width ? pxToRem(width) : undefined}
	use:tooltip={{
		text: help,
		delay: helpShowDelay
	}}
	bind:this={element}
	disabled={disabled || loading}
	on:click
	on:mousedown
	on:contextmenu
	{type}
	{id}
	tabindex={clickable ? tabindex : -1}
>
	{#if $$slots.default}
		<span
			class="label text-semibold"
			class:text-base-12={size === 'button' || size === 'cta'}
			class:text-base-11={size === 'tag'}
		>
			<slot />
		</span>
	{/if}

	{#if icon || loading}
		<div class="btn-icon">
			{#if loading}
				<Icon name="spinner" spinnerRadius={4.5} />
			{:else if icon}
				<Icon name={icon} />
			{/if}
		</div>
	{/if}
</button>

<style lang="postcss">
	.btn {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-m);
		border: 1px solid transparent;
		cursor: pointer;
		color: var(--btn-text-clr);
		background: var(--btn-bg);
		transition:
			background var(--transition-fast),
			opacity var(--transition-fast),
			color var(--transition-fast);
		-webkit-transform-style: preserve-3d;
		-webkit-backface-visibility: hidden;

		&:disabled {
			cursor: default;
			pointer-events: none;
			opacity: 0.5;
		}
		&.wide {
			display: flex;
			width: 100%;
		}
		&.grow {
			flex-grow: 1;
		}
		&.reversed-direction {
			flex-direction: row-reverse;
		}
		&.not-clickable {
			cursor: default;
			pointer-events: none;
		}
		&.shrinkable {
			overflow: hidden;
			width: fit-content;

			& .label {
				display: inline-block;
				overflow: hidden;
				text-overflow: ellipsis;
			}
		}
	}

	.label {
		display: inline-flex;
		white-space: nowrap;
	}

	.btn-icon {
		flex-shrink: 0;
		display: flex;
		opacity: var(--icon-opacity);
		transition: opacity var(--transition-fast);
		/* in order to fix the transition flickering bug in Safari */
		-webkit-transform: translateZ(0);
	}

	/* STYLES */
	.neutral {
		--icon-opacity: 0.6;
		/* kind */
		&.soft {
			--btn-text-clr: var(--clr-theme-ntrl-on-soft);
			--btn-bg: var(--clr-theme-ntrl-soft);

			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.8;
				--btn-bg: var(--clr-theme-ntrl-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-ntrl-on-element);
			--btn-bg: var(--clr-theme-ntrl-element);

			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.8;
				--btn-bg: var(--clr-theme-ntrl-element-hover);
			}
		}
	}

	.ghost {
		--icon-opacity: 0.6;
		--btn-text-clr: var(--clr-theme-ntrl-on-soft);
		--btn-bg: transparent;

		&:not(.not-clickable, &:disabled):hover {
			--icon-opacity: 0.8;
			--btn-bg: var(--clr-bg-1-muted);
		}

		&.outline {
			border: 1px solid var(--clr-border-2);
		}

		&.dashed {
			border-style: dashed;
		}

		&.solidBackground {
			background: var(--clr-bg-1);
		}
	}

	.pop {
		--icon-opacity: 0.8;

		&.soft {
			--btn-text-clr: var(--clr-theme-pop-on-soft);
			--btn-bg: var(--clr-theme-pop-soft);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-pop-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-pop-on-element);
			--btn-bg: var(--clr-theme-pop-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-pop-element-hover);
			}
		}
	}

	.success {
		--icon-opacity: 0.8;

		&.soft {
			--btn-text-clr: var(--clr-theme-succ-on-soft);
			--btn-bg: var(--clr-theme-succ-soft);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-succ-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-succ-on-element);
			--btn-bg: var(--clr-theme-succ-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-succ-element-hover);
			}
		}
	}

	.error {
		--icon-opacity: 0.8;

		&.soft {
			--btn-text-clr: var(--clr-theme-err-on-soft);
			--btn-bg: var(--clr-theme-err-soft);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-err-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-err-on-element);
			--btn-bg: var(--clr-theme-err-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-err-element-hover);
			}
		}
	}

	.warning {
		--icon-opacity: 0.8;

		&.soft {
			--btn-text-clr: var(--clr-theme-warn-on-soft);
			--btn-bg: var(--clr-theme-warn-soft);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-warn-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-warn-on-element);
			--btn-bg: var(--clr-theme-warn-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-warn-element-hover);
			}
		}
	}

	.purple {
		--icon-opacity: 0.8;

		&.soft {
			--btn-text-clr: var(--clr-theme-purp-on-soft);
			--btn-bg: var(--clr-theme-purp-soft);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-purp-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-purp-on-element);
			--btn-bg: var(--clr-theme-purp-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-purp-element-hover);
			}
		}
	}

	/* SIZE MODIFIERS */

	.btn.tag-size {
		height: var(--size-tag);
		padding: 2px 4px;

		& .label {
			padding: 0 4px;
		}
	}

	.btn.button-size {
		height: var(--size-button);
		padding: 4px 6px;

		& .label {
			padding: 0 4px;
		}
	}

	.btn.cta-size {
		height: var(--size-cta);
		padding: 6px 8px;

		& .label {
			padding: 0 6px;
		}
	}

	/* FIXED WIDTH */

	.btn.fixed-width {
		&.tag-size {
			width: var(--size-tag);
		}

		&.button-size {
			width: var(--size-button);
		}

		&.cta-size {
			width: var(--size-cta);
		}
	}

	/* DROPDOWN */
	.is-dropdown {
		&:first-of-type {
			flex: 1;
			border-top-right-radius: 0;
			border-bottom-right-radius: 0;
			border-right: none;

			&.pop,
			&.success,
			&.error,
			&.warning,
			&.purple {
				&:after {
					content: '';
					background-color: currentColor;
					z-index: var(--z-lifted);
					position: absolute;
					top: 0;
					right: 0;
					width: 1px;
					height: 100%;
					opacity: 0.2;
				}
			}
		}

		&:last-of-type {
			border-top-left-radius: 0;
			border-bottom-left-radius: 0;
		}
	}
</style>
