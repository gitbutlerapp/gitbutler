<script lang="ts" context="module">
	export type TagStyle = 'neutral' | 'ghost' | 'pop' | 'success' | 'error' | 'warning' | 'purple';
	export type TagKind = 'soft' | 'solid';
</script>

<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { tooltip } from '$lib/utils/tooltip';
	import type iconsJson from '$lib/icons/icons.json';

	// Interaction props
	export let help = '';
	export let disabled = false;
	export let clickable = false;
	export let loading = false;
	// Layout props
	export let shrinkable = false;
	export let verticalOrientation = false;
	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let reversedDirection = false;
	// Style props
	export let style: TagStyle = 'neutral';
	export let kind: TagKind = 'soft';
</script>

<div
	class="tag text-base-11 text-semibold {style} {kind}"
	class:disabled
	class:shrinkable
	class:reversedDirection
	class:verticalOrientation
	class:not-button={!clickable}
	role={clickable ? 'button' : undefined}
	use:tooltip={help}
	on:click
	on:mousedown
	on:contextmenu
>
	<span class="label" class:verticalLabel={verticalOrientation}>
		<slot />
	</span>
	{#if loading}
		<Icon name="spinner" />
	{:else if icon}
		<div class="icon" class:verticalIcon={verticalOrientation}>
			<Icon name={icon} spinnerRadius={3.5} />
		</div>
	{/if}
</div>

<style lang="postcss">
	/* BASE */
	.tag {
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--size-control-tag);
		padding: var(--size-2) var(--size-4);
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);

		/* component variables */
		--soft-bg-ratio: transparent 80%;
		--soft-hover-ratio: transparent 75%;
	}
	.icon {
		flex-shrink: 0;
	}
	.label {
		white-space: nowrap;
		display: inline-block;
		padding: 0 var(--size-2);
	}

	/* STYLES */
	.neutral {
		/* kind */
		&.soft {
			color: var(--clr-scale-ntrl-30);
			background: color-mix(in srgb, var(--clr-core-ntrl-50), var(--soft-bg-ratio));
			/* if button */
			&:not(.not-button):hover {
				color: var(--clr-scale-ntrl-30);
				background: color-mix(in srgb, var(--clr-core-ntrl-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-scale-ntrl-100);
			background: var(--clr-scale-ntrl-30);
			/* if button */
			&:not(.not-button):hover {
				background: var(--clr-scale-ntrl-30);
			}
		}
	}

	.ghost {
		&.soft,
		&.solid {
			color: var(--clr-scale-ntrl-30);
			background: transparent;
			&:not(.not-button):hover {
				color: var(--clr-scale-ntrl-30);
				background: color-mix(in srgb, transparent, var(--darken-tint-light));
			}
		}

		&.solid {
			box-shadow: inset 0 0 0 1px var(--clr-scale-ntrl-60);

			&:not(.not-button):hover {
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
			&:not(.not-button):hover {
				color: var(--clr-scale-pop-10);
				background: color-mix(in srgb, var(--clr-core-pop-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-theme-pop-on-element);
			background: var(--clr-theme-pop-element);
			/* if button */
			&:not(.not-button):hover {
				background: color-mix(in srgb, var(--clr-theme-pop-element), var(--darken-mid));
			}
		}
	}

	.success {
		&.soft {
			color: var(--clr-scale-succ-20);
			background: color-mix(in srgb, var(--clr-core-succ-50), var(--soft-bg-ratio));
			/* if button */
			&:not(.not-button):hover {
				color: var(--clr-scale-succ-10);
				background: color-mix(in srgb, var(--clr-core-succ-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-theme-succ-on-element);
			background: var(--clr-theme-succ-element);
			/* if button */
			&:not(.not-button):hover {
				background: color-mix(in srgb, var(--clr-theme-succ-element), var(--darken-mid));
			}
		}
	}

	.error {
		&.soft {
			color: var(--clr-scale-err-20);
			background: color-mix(in srgb, var(--clr-core-err-50), var(--soft-bg-ratio));
			/* if button */
			&:not(.not-button):hover {
				color: var(--clr-scale-err-10);
				background: color-mix(in srgb, var(--clr-core-err-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-theme-err-on-element);
			background: var(--clr-theme-err-element);
			/* if button */
			&:not(.not-button):hover {
				background: color-mix(in srgb, var(--clr-theme-err-element), var(--darken-mid));
			}
		}
	}

	.warning {
		&.soft {
			color: var(--clr-scale-warn-20);
			background: color-mix(in srgb, var(--clr-core-warn-50), var(--soft-bg-ratio));
			/* if button */
			&:not(.not-button):hover {
				color: var(--clr-scale-warn-10);
				background: color-mix(in srgb, var(--clr-core-warn-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-theme-warn-on-element);
			background: var(--clr-theme-warn-element);
			/* if button */
			&:not(.not-button):hover {
				background: color-mix(in srgb, var(--clr-theme-warn-element), var(--darken-mid));
			}
		}
	}

	.purple {
		&.soft {
			color: var(--clr-scale-purple-20);
			background: color-mix(in srgb, var(--clr-core-purple-50), var(--soft-bg-ratio));
			/* if button */
			&:not(.not-button):hover {
				color: var(--clr-scale-purple-10);
				background: color-mix(in srgb, var(--clr-core-purple-50), var(--soft-hover-ratio));
			}
		}
		&.solid {
			color: var(--clr-theme-purple-on-element);
			background: var(--clr-theme-purple-element);
			/* if button */
			&:not(.not-button):hover {
				background: color-mix(in srgb, var(--clr-theme-purple-element), var(--darken-mid));
			}
		}
	}

	/* modifiers */

	.not-button {
		cursor: default;
		user-select: none;
	}

	.disabled {
		pointer-events: none;
		opacity: 0.6;
	}

	.reversedDirection {
		flex-direction: row-reverse;
	}

	.shrinkable {
		overflow: hidden;

		& span {
			overflow: hidden;
			text-overflow: ellipsis;
		}
	}

	.verticalOrientation {
		writing-mode: vertical-rl;
		height: max-content;
		width: var(--size-control-tag);
		padding: var(--size-4) var(--size-2);
		transform: rotate(180deg);
	}

	.verticalIcon {
		transform: rotate(90deg);
	}

	.verticalLabel {
		padding: var(--size-2) 0;
	}
</style>
