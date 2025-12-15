<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import SkeletonBone from '$components/SkeletonBone.svelte';
	import Tooltip from '$components/Tooltip.svelte';
	import type iconsJson from '$lib/data/icons.json';
	import type { ComponentColorType } from '$lib/utils/colorTypes';
	import type { Snippet } from 'svelte';

	interface Props {
		testId?: string;
		style?: ComponentColorType;
		kind?: 'solid' | 'soft';
		size?: 'icon' | 'tag';
		class?: string;
		icon?: keyof typeof iconsJson | undefined;
		tooltip?: string;
		skeleton?: boolean;
		children?: Snippet;
		onclick?: (e: MouseEvent) => void;
		reversedDirection?: boolean;
	}

	const {
		testId,
		style = 'gray',
		kind = 'solid',
		size = 'icon',
		class: className = '',
		icon,
		tooltip,
		skeleton,
		children,
		onclick,
		reversedDirection
	}: Props = $props();
</script>

{#if skeleton}
	<SkeletonBone
		radius="3rem"
		width={size === 'icon' ? 'var(--size-icon)' : 'var(--size-tag)'}
		height={size === 'icon' ? 'var(--size-icon)' : 'var(--size-tag)'}
	/>
{:else}
	<Tooltip text={tooltip}>
		<div
			role="presentation"
			data-testid={testId}
			class="badge {style} {kind} {size}-size {className}"
			class:reversed={reversedDirection}
			{onclick}
		>
			{#if children}
				<span class="badge__label text-11 text-semibold">{@render children()}</span>
			{/if}
			{#if icon}
				<i class="badge__icon">
					<Icon name={icon} />
				</i>
			{/if}
		</div>
	</Tooltip>
{/if}

<style lang="postcss">
	.badge {
		display: inline-flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		border-radius: 20px;
		line-height: 90%;
		text-align: center;

		/* SOLID */
		&.gray.solid {
			background-color: var(--clr-theme-gray-element);
			color: var(--clr-theme-gray-on-element);
		}

		&.pop.solid {
			background-color: var(--clr-theme-pop-element);
			color: var(--clr-theme-pop-on-element);
		}

		&.success.solid {
			background-color: var(--clr-theme-succ-element);
			color: var(--clr-theme-succ-on-element);
		}

		&.warning.solid {
			background-color: var(--clr-theme-warn-element);
			color: var(--clr-theme-warn-on-element);
		}

		&.error.solid {
			background-color: var(--clr-theme-danger-element);
			color: var(--clr-theme-danger-on-element);
		}

		&.purple.solid {
			background-color: var(--clr-theme-purp-element);
			color: var(--clr-theme-purp-on-element);
		}

		/* SOFT */
		&.gray.soft {
			background-color: var(--clr-theme-gray-soft);
			color: var(--clr-theme-gray-on-soft);
		}

		&.pop.soft {
			background-color: var(--clr-theme-pop-soft);
			color: var(--clr-theme-pop-on-soft);
		}

		&.success.soft {
			background-color: var(--clr-theme-succ-soft);
			color: var(--clr-theme-succ-on-soft);
		}

		&.warning.soft {
			background-color: var(--clr-theme-warn-soft);
			color: var(--clr-theme-warn-on-soft);
		}

		&.error.soft {
			background-color: var(--clr-theme-danger-soft);
			color: var(--clr-theme-danger-on-soft);
		}

		&.purple.soft {
			background-color: var(--clr-theme-purp-soft);
			color: var(--clr-theme-purp-on-soft);
		}

		/* SIZE */
		&.icon-size {
			min-width: var(--size-icon);
			height: var(--size-icon);
			gap: 1px;

			& .badge__label {
				padding: 0 2px 0 5px;
			}

			/* When no icon, use equal padding */
			&:not(:has(.badge__icon)) .badge__label {
				padding: 0 5px;
			}
		}

		&.tag-size {
			min-width: var(--size-tag);
			height: var(--size-tag);
			gap: 2px;

			& .badge__label {
				padding: 0 2px 0 8px;
			}

			& .badge__icon {
				padding-right: 4px;
				padding-left: 0;
			}

			/* When no icon, use equal padding */
			&:not(:has(.badge__icon)) .badge__label {
				padding: 0 8px;
			}
		}

		&.reversed {
			flex-direction: row-reverse;

			&.icon-size {
				& .badge__label {
					padding: 0 5px 0 2px;
				}

				& .badge__icon {
					padding-right: 0;
					padding-left: 2px;
				}
			}

			&.tag-size {
				& .badge__label {
					padding: 0 8px 0 2px;
				}

				& .badge__icon {
					padding-right: 0;
					padding-left: 4px;
				}
			}
		}
	}

	.badge__label {
		display: flex;
		line-height: var(--size-icon);
		white-space: nowrap;
	}

	.badge__icon {
		display: flex;
		opacity: 0.7;
	}
</style>
