<script lang="ts" context="module">
	export type ButtonStyle =
		| 'neutral'
		| 'ghost'
		| 'pop'
		| 'success'
		| 'error'
		| 'warning'
		| 'purple';
	export type ButtonKind = 'soft' | 'solid';
</script>

<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { tooltip } from '$lib/utils/tooltip';
	import { onMount } from 'svelte';
	import type iconsJson from '$lib/icons/icons.json';

	// Interaction props
	export let element: HTMLAnchorElement | HTMLButtonElement | HTMLElement | null = null;
	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let isDropdownChild = false;
	export let disabled = false;
	export let clickable = false;
	export let id: string | undefined = undefined;
	export let loading = false;
	export let tabindex = 0;
	export let help = '';
	export let type: 'submit' | 'reset' | undefined = undefined;
	// Layout props
	export let width: number | undefined = undefined;
	export let size: 'medium' | 'large' = 'medium';
	export let reversedDirection: boolean = false;
	export let wide = false;
	export let grow = false;
	export let align: 'flex-start' | 'center' | 'flex-end' | 'stretch' | 'baseline' | 'auto' = 'auto';
	// Style props
	export let style: ButtonStyle = 'neutral';
	export let kind: ButtonKind = 'soft';

	const SLOTS = $$props.$$slots;

	onMount(() => {
		if (!element) return;
		element.ariaLabel = element.innerText?.trim();
	});
</script>

<button
	class="btn {style} {kind} {size}"
	class:reversed-direction={reversedDirection}
	class:wide
	class:grow
	class:not-button={clickable}
	class:is-dropdown={isDropdownChild}
	style:align-self={align}
	style:width={width ? pxToRem(width) : undefined}
	use:tooltip={help}
	bind:this={element}
	disabled={disabled || loading}
	on:click
	on:mousedown
	{type}
	{id}
	tabindex={clickable ? -1 : tabindex}
>
	{#if SLOTS}
		<span class="label text-base-12 text-semibold">
			<slot />
		</span>
	{/if}
	{#if icon && !loading}
		<Icon name={icon} />
	{:else if loading}
		<Icon name="spinner" />
	{/if}
</button>

<style lang="postcss">
	.btn {
		z-index: 1;
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: var(--size-4) var(--size-6);
		border-radius: var(--radius-m);
		flex-shrink: 0;
		gap: var(--size-2);
		border: 1px solid transparent;
		transition:
			background var(--transition-fast),
			opacity var(--transition-fast),
			color var(--transition-fast);
		cursor: pointer;

		/* component variables */
		--soft-bg-ratio: transparent 80%;
		--soft-hover-ratio: transparent 75%;

		&:disabled {
			cursor: default;
			pointer-events: none;
			opacity: 0.5;

			&.neutral.solid,
			&.pop.solid,
			&.success.solid,
			&.error.solid,
			&.warning.solid,
			&.purple.solid {
				color: color-mix(in srgb, var(--clr-scale-ntrl-0), transparent 40%);
				background: color-mix(in srgb, var(--clr-scale-ntrl-40), transparent 80%);
			}

			&.neutral.soft,
			&.pop.soft,
			&.success.soft,
			&.error.soft,
			&.warning.soft,
			&.purple.soft {
				color: var(--clr-scale-ntrl-40);
				background: color-mix(in srgb, var(--clr-scale-ntrl-50), transparent 80%);
			}
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
		&.not-button {
			cursor: default;
			pointer-events: none;
		}
	}
	.label {
		display: inline-flex;
		padding: 0 var(--size-2);
	}

	/* STYLES */
	.neutral {
		/* kind */
		&.soft {
			color: var(--clr-scale-ntrl-30);
			background: color-mix(in srgb, var(--clr-core-ntrl-50), var(--soft-bg-ratio));
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-ntrl-30);
				background: color-mix(in srgb, var(--clr-core-ntrl-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-scale-ntrl-100);
			background: var(--clr-scale-ntrl-30);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: var(--clr-scale-ntrl-30);
			}
		}
	}

	.ghost {
		&.soft,
		&.solid {
			color: var(--clr-scale-ntrl-30);
			background: transparent;
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-ntrl-30);
				background: color-mix(in srgb, transparent, var(--darken-tint-light));
			}
		}

		&.solid {
			border: 1px solid var(--clr-scale-ntrl-60);

			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-ntrl-30);
				background: color-mix(in srgb, transparent, var(--darken-tint-light));
			}
		}
	}

	.pop {
		&.soft {
			color: var(--clr-scale-pop-20);
			background: color-mix(in srgb, var(--clr-core-pop-50), var(--soft-bg-ratio));
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-pop-10);
				background: color-mix(in srgb, var(--clr-core-pop-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-theme-pop-on-element);
			background: var(--clr-theme-pop-element);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: color-mix(in srgb, var(--clr-theme-pop-element), var(--darken-mid));
			}
		}
	}

	.success {
		&.soft {
			color: var(--clr-scale-succ-20);
			background: color-mix(in srgb, var(--clr-core-succ-50), var(--soft-bg-ratio));
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-succ-10);
				background: color-mix(in srgb, var(--clr-core-succ-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-theme-succ-on-element);
			background: var(--clr-theme-succ-element);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: color-mix(in srgb, var(--clr-theme-succ-element), var(--darken-mid));
			}
		}
	}

	.error {
		&.soft {
			color: var(--clr-scale-err-20);
			background: color-mix(in srgb, var(--clr-core-err-50), var(--soft-bg-ratio));
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-err-10);
				background: color-mix(in srgb, var(--clr-core-err-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-theme-err-on-element);
			background: var(--clr-theme-err-element);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: color-mix(in srgb, var(--clr-theme-err-element), var(--darken-mid));
			}
		}
	}

	.warning {
		&.soft {
			color: var(--clr-scale-warn-20);
			background: color-mix(in srgb, var(--clr-core-warn-50), var(--soft-bg-ratio));
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-warn-10);
				background: color-mix(in srgb, var(--clr-core-warn-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-theme-warn-on-element);
			background: var(--clr-theme-warn-element);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: color-mix(in srgb, var(--clr-theme-warn-element), var(--darken-mid));
			}
		}
	}

	.purple {
		&.soft {
			color: var(--clr-scale-purple-20);
			background: color-mix(in srgb, var(--clr-core-purple-50), var(--soft-bg-ratio));
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-purple-10);
				background: color-mix(in srgb, var(--clr-core-purple-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-theme-purple-on-element);
			background: var(--clr-theme-purple-element);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: color-mix(in srgb, var(--clr-theme-purple-element), var(--darken-mid));
			}
		}
	}

	/* SIZE MODIFIERS */

	.btn.medium {
		height: var(--size-control-button);
		min-width: var(--size-control-button);
		padding: var(--size-4) var(--size-6);
	}

	.btn.large {
		height: var(--size-control-cta);
		min-width: var(--size-control-cta);
		padding: var(--size-6) var(--size-8);
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
					z-index: 2;
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
