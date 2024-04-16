<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
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
	export let type: 'submit' | 'reset' | undefined = undefined;
	// Layout props
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
	// Additional elements
	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let help = '';
	export let badgeLabel: string | number | undefined = undefined;
	export let badgeIcon: keyof typeof iconsJson | undefined = undefined;

	const SLOTS = $$props.$$slots;
</script>

<button
	class="btn focus-state {style} {kind} {size}"
	class:reversed-direction={reversedDirection}
	class:wide
	class:grow
	class:not-clickable={!clickable}
	class:fixed-width={!SLOTS}
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
	tabindex={clickable ? tabindex : -1}
>
	{#if SLOTS}
		<span
			class="label text-semibold"
			class:text-base-12={size == 'button' || size == 'cta'}
			class:text-base-11={size == 'tag'}
		>
			<slot />
		</span>
	{/if}

	{#if badgeLabel}
		<div class="badge">
			<span class="text-base-11 text-semibold badge-label">
				{badgeLabel}
			</span>{#if badgeIcon}
				<div class="badge-icon">
					<Icon name={badgeIcon} />
				</div>{/if}
		</div>
	{/if}

	{#if icon && !loading}
		<Icon name={icon} />
	{:else if loading}
		<Icon name="spinner" />
	{/if}
</button>

<style lang="postcss">
	.btn {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: var(--size-4) var(--size-6);
		border-radius: var(--radius-m);
		flex-shrink: 0;
		gap: var(--size-4);
		border: 1px solid transparent;
		transition:
			background var(--transition-fast),
			opacity var(--transition-fast),
			color var(--transition-fast);
		cursor: pointer;
		color: var(--btn-clr);
		background: var(--btn-bg);

		&:disabled {
			cursor: default;
			pointer-events: none;
			/* opacity: 0.5; */

			&.neutral.solid,
			&.pop.solid,
			&.success.solid,
			&.error.solid,
			&.warning.solid,
			&.purple.solid {
				/* color: var(--clr-bg-on-muted); */
				--btn-clr: var(--clr-bg-on-muted);
				--btn-bg: oklch(from var(--clr-scale-ntrl-60) l c h / 0.2);

				& .badge {
					--btn-bg: var(--clr-scale-ntrl-100);
				}
			}

			&.neutral.soft,
			&.pop.soft,
			&.success.soft,
			&.error.soft,
			&.warning.soft,
			&.purple.soft {
				--btn-clr: var(--clr-bg-on-muted);
				--btn-bg: oklch(from var(--clr-scale-ntrl-60) l c h / 0.2);
			}

			&.ghost {
				--btn-clr: var(--clr-bg-on-muted);
			}

			&.ghost.solid {
				border-color: oklch(from var(--clr-scale-ntrl-0) l c h / 0.1);
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
		&.not-clickable {
			cursor: default;
			pointer-events: none;
		}
	}

	.label {
		display: inline-flex;
	}

	/* BADGE */
	.badge {
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--size-control-icon);
		min-width: var(--size-control-icon);
		padding: 0 var(--size-4);
		border-radius: var(--radius-s);
		background: var(--btn-clr);
	}

	.badge-label {
		transform: translateY(0.031rem);
		color: var(--btn-bg);
	}

	.badge-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-icon);
		height: var(--size-control-icon);
		margin-right: -0.125rem;
		color: white;
	}

	/* STYLES */
	.neutral {
		/* kind */
		&.soft {
			--btn-clr: var(--clr-scale-ntrl-40);
			--btn-bg: oklch(from var(--clr-core-ntrl-60) l c h / 0.15);

			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-clr: var(--clr-scale-ntrl-20);
				--btn-bg: oklch(from var(--clr-core-ntrl-50) l c h / 0.18);
			}

			& .badge {
				--btn-bg: var(--clr-scale-ntrl-100);
			}
		}
		&.solid {
			--btn-clr: var(--clr-scale-ntrl-100);
			--btn-bg: var(--clr-scale-ntrl-30);

			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: var(--clr-scale-ntrl-30);
			}
		}
	}

	.ghost {
		&.soft,
		&.solid {
			--btn-clr: var(--clr-scale-ntrl-40);
			--btn-bg: transparent;

			&:not(.not-clickable, &:disabled):hover {
				--btn-clr: var(--clr-scale-ntrl-20);
				--btn-bg: var(--clr-bg-muted);
			}

			& .badge {
				--btn-bg: var(--clr-scale-ntrl-100);
			}
		}

		&.solid {
			border: 1px solid oklch(from var(--clr-scale-ntrl-0) l c h / 0.2);

			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: oklch(from var(--clr-core-ntrl-60) l c h / 0.1);
			}
		}
	}

	.pop {
		&.soft {
			--btn-clr: var(--clr-theme-pop-on-container);
			--btn-bg: var(--clr-scale-pop-80);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: oklch(from var(--clr-scale-pop-80) var(--hover-state-ratio) c h);
			}

			& .badge {
				--btn-bg: var(--clr-scale-ntrl-100);
			}
		}
		&.solid {
			--btn-clr: var(--clr-theme-pop-on-element);
			--btn-bg: var(--clr-theme-pop-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: oklch(from var(--clr-theme-pop-element) var(--hover-state-ratio) c h);
			}
		}
	}

	.success {
		&.soft {
			--btn-clr: var(--clr-theme-succ-on-container);
			--btn-bg: var(--clr-scale-succ-80);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: oklch(from var(--clr-scale-succ-80) var(--hover-state-ratio) c h);
			}

			& .badge {
				--btn-bg: var(--clr-scale-ntrl-100);
			}
		}
		&.solid {
			--btn-clr: var(--clr-theme-succ-on-element);
			--btn-bg: var(--clr-theme-succ-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: oklch(from var(--clr-theme-succ-element) var(--hover-state-ratio) c h);
			}
		}
	}

	.error {
		&.soft {
			--btn-clr: var(--clr-theme-err-on-container);
			--btn-bg: var(--clr-scale-err-80);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: oklch(from var(--clr-scale-err-80) var(--hover-state-ratio) c h);
			}

			& .badge {
				--btn-bg: var(--clr-scale-ntrl-100);
			}
		}
		&.solid {
			--btn-clr: var(--clr-theme-err-on-element);
			--btn-bg: var(--clr-theme-err-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: oklch(from var(--clr-theme-err-element) var(--hover-state-ratio) c h);
			}
		}
	}

	.warning {
		&.soft {
			--btn-clr: var(--clr-theme-warn-on-container);
			--btn-bg: var(--clr-scale-warn-80);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: oklch(from var(--clr-scale-warn-80) var(--hover-state-ratio) c h);
			}

			& .badge {
				--btn-bg: var(--clr-scale-ntrl-100);
			}
		}
		&.solid {
			--btn-clr: var(--clr-theme-warn-on-element);
			--btn-bg: var(--clr-theme-warn-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: oklch(from var(--clr-theme-warn-element) var(--hover-state-ratio) c h);
			}
		}
	}

	.purple {
		&.soft {
			--btn-clr: var(--clr-theme-purp-on-container);
			--btn-bg: var(--clr-scale-purp-80);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: oklch(from var(--clr-scale-purp-80) var(--hover-state-ratio) c h);
			}

			& .badge {
				--btn-bg: var(--clr-scale-ntrl-100);
			}
		}
		&.solid {
			--btn-clr: var(--clr-theme-purp-on-element);
			--btn-bg: var(--clr-theme-purp-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--btn-bg: oklch(from var(--clr-theme-purp-element) var(--hover-state-ratio) c h);
			}
		}
	}

	/* SIZE MODIFIERS */

	.btn.tag {
		height: var(--size-control-tag);
		min-width: var(--size-control-tag);
		padding: var(--size-2) var(--size-4);
	}

	.btn.button {
		height: var(--size-control-button);
		min-width: var(--size-control-button);
		padding: var(--size-4) var(--size-8);
	}

	.btn.cta {
		height: var(--size-control-cta);
		min-width: var(--size-control-cta);
		padding: var(--size-6) var(--size-8);
	}

	/* FIXED WIDTH */

	.btn.fixed-width {
		&.tag {
			width: var(--size-control-tag);
			padding: var(--size-2);
		}

		&.button {
			width: var(--size-control-button);
			padding: var(--size-4);
		}

		&.cta {
			width: var(--size-control-cta);
			padding: var(--size-6);
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
