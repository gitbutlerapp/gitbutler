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
	export let disabled = false;
	export let clickable = false;
	export let id: string | undefined = undefined;
	export let loading = false;
	export let tabindex = 0;
	export let type: 'submit' | 'reset' | undefined = undefined;
	// Layout props
	export let width: number | undefined = undefined;
	export let size: 'medium' | 'large' = 'medium';
	export let reversedDirection: boolean = false;
	export let wide = false;
	export let grow = false;
	export let align: 'flex-start' | 'center' | 'flex-end' | 'stretch' | 'baseline' | 'auto' = 'auto';
	export let isDropdownChild = false;
	// Style props
	export let style: ButtonStyle = 'neutral';
	export let kind: ButtonKind = 'soft';
	// Additional elements
	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let help = '';
	export let badgeLabel: string | number | undefined = undefined;
	export let badgeIcon: keyof typeof iconsJson | undefined = undefined;

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
		z-index: 1;
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

		/* component variables */
		--label-color: var(--clr-scale-ntrl-100);
		--soft-bg-ratio: transparent 80%;
		--soft-hover-ratio: transparent 75%;

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
				color: var(--clr-bg-on-muted);
				background: oklch(from var(--clr-scale-ntrl-60) l c h / 0.15);
			}

			&.neutral.soft,
			&.pop.soft,
			&.success.soft,
			&.error.soft,
			&.warning.soft,
			&.purple.soft {
				color: var(--clr-bg-on-muted);
				background: oklch(from var(--clr-scale-ntrl-60) l c h / 0.15);
			}

			&.ghost {
				color: var(--clr-bg-on-muted);
			}

			&.ghost.solid {
				border: 1px solid oklch(from var(--clr-scale-ntrl-0) l c h / 0.1);
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

	/* BADGE */
	.badge {
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--size-control-icon);
		min-width: var(--size-control-icon);
		padding: 0 var(--size-4);
		border-radius: var(--radius-s);
	}

	.badge-label {
		transform: translateY(0.031rem);
		color: var(--label-color);
	}

	.badge-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-icon);
		height: var(--size-control-icon);
		margin-right: -0.125rem;
		color: var(--label-color);
	}

	/* STYLES */
	.neutral {
		/* kind */
		&.soft {
			color: var(--clr-scale-ntrl-30);
			background: oklch(from var(--clr-core-ntrl-60) l c h / 0.15);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-ntrl-30);
				background: oklch(from var(--clr-core-ntrl-50) l c h / 0.18);
			}

			& .badge {
				--label-color: var(--clr-scale-ntrl-100);
				background: var(--clr-scale-ntrl-30);
			}
		}
		&.solid {
			color: var(--clr-scale-ntrl-100);
			background: var(--clr-scale-ntrl-30);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: var(--clr-scale-ntrl-30);
			}

			& .badge {
				--label-color: var(--clr-scale-ntrl-30);
				background: var(--clr-scale-ntrl-100);
			}
		}
	}

	.ghost {
		&.soft,
		&.solid {
			color: var(--clr-scale-ntrl-40);
			background: transparent;
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-ntrl-20);
				background: oklch(from var(--clr-core-ntrl-60) l c h / 0.15);
			}

			& .badge {
				--label-color: var(--clr-scale-ntrl-100);
				background: var(--clr-scale-ntrl-30);
			}
		}

		&.solid {
			border: 1px solid oklch(from var(--clr-scale-ntrl-0) l c h / 0.2);

			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-ntrl-30);
				background: oklch(from var(--clr-core-ntrl-60) l c h / 0.1);
			}
		}
	}

	.pop {
		&.soft {
			color: var(--clr-theme-pop-on-container);
			background: var(--clr-scale-pop-80);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-pop-10);
				background: oklch(from var(--clr-scale-pop-80) var(--hover-state-ratio) c h);
			}

			& .badge {
				--label-color: var(--clr-scale-ntrl-100);
				background: var(--clr-scale-pop-20);
			}
		}
		&.solid {
			color: var(--clr-theme-pop-on-element);
			background: var(--clr-theme-pop-element);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: oklch(from var(--clr-theme-pop-element) var(--hover-state-ratio) c h);
			}

			& .badge {
				--label-color: var(--clr-theme-pop-element);
				background: var(--clr-core-ntrl-100);
			}
		}
	}

	.success {
		&.soft {
			color: var(--clr-theme-succ-on-container);
			background: var(--clr-scale-succ-80);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-succ-10);
				background: oklch(from var(--clr-scale-succ-80) var(--hover-state-ratio) c h);
			}

			& .badge {
				--label-color: var(--clr-scale-ntrl-100);
				background: var(--clr-scale-succ-20);
			}
		}
		&.solid {
			color: var(--clr-theme-succ-on-element);
			background: var(--clr-theme-succ-element);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: oklch(from var(--clr-theme-succ-element) var(--hover-state-ratio) c h);
			}

			& .badge {
				--label-color: var(--clr-theme-succ-element);
				background: var(--clr-core-ntrl-100);
			}
		}
	}

	.error {
		&.soft {
			color: var(--clr-theme-err-on-container);
			background: var(--clr-scale-err-80);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-err-10);
				background: oklch(from var(--clr-scale-err-80) var(--hover-state-ratio) c h);
			}

			& .badge {
				--label-color: var(--clr-scale-ntrl-100);
				background: var(--clr-scale-err-20);
			}
		}
		&.solid {
			color: var(--clr-theme-err-on-element);
			background: var(--clr-theme-err-element);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: oklch(from var(--clr-theme-err-element) var(--hover-state-ratio) c h);
			}

			& .badge {
				--label-color: var(--clr-theme-err-element);
				background: var(--clr-core-ntrl-100);
			}
		}
	}

	.warning {
		&.soft {
			color: var(--clr-theme-warn-on-container);
			background: var(--clr-scale-warn-80);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-warn-10);
				background: oklch(from var(--clr-scale-warn-80) var(--hover-state-ratio) c h);
			}

			& .badge {
				--label-color: var(--clr-scale-ntrl-100);
				background: var(--clr-scale-warn-20);
			}
		}
		&.solid {
			color: var(--clr-theme-warn-on-element);
			background: var(--clr-theme-warn-element);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: oklch(from var(--clr-theme-warn-element) var(--hover-state-ratio) c h);
			}

			& .badge {
				--label-color: var(--clr-theme-warn-element);
				background: var(--clr-core-ntrl-100);
			}
		}
	}

	.purple {
		&.soft {
			color: var(--clr-theme-purp-on-container);
			background: var(--clr-scale-purp-80);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				color: var(--clr-scale-purp-10);
				background: oklch(from var(--clr-scale-purp-80) var(--hover-state-ratio) c h);
			}

			& .badge {
				--label-color: var(--clr-scale-ntrl-100);
				background: var(--clr-scale-purp-20);
			}
		}
		&.solid {
			color: var(--clr-theme-purp-on-element);
			background: var(--clr-theme-purp-element);
			/* if button */
			&:not(.not-button, &:disabled):hover {
				background: oklch(from var(--clr-theme-purp-element) var(--hover-state-ratio) c h);
			}

			& .badge {
				--label-color: var(--clr-theme-purp-element);
				background: var(--clr-core-ntrl-100);
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
