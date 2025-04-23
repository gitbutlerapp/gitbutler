<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import Tooltip from '$lib/Tooltip.svelte';
	import type iconsJson from '$lib/data/icons.json';
	import type { ComponentColorType } from '$lib/utils/colorTypes';
	import type { Snippet } from 'svelte';

	interface Props {
		style?: ComponentColorType;
		kind?: 'solid' | 'soft';
		size?: 'icon' | 'tag';
		icon?: keyof typeof iconsJson | undefined;
		reversedDirection?: boolean;
		tooltip?: string;
		children?: Snippet;
	}

	const {
		style = 'neutral',
		kind = 'solid',
		size = 'icon',
		icon,
		reversedDirection,
		tooltip,
		children
	}: Props = $props();
</script>

<Tooltip text={tooltip}>
	<div class="badge {style} {kind} {size}-size" class:reversedDirection>
		{#if children}
			<span
				class="badge__label text-bold"
				class:text-10={size === 'icon'}
				class:text-11={size === 'tag'}>{@render children()}</span
			>
		{/if}
		{#if icon}
			<Icon name={icon} />
		{/if}
	</div>
</Tooltip>

<style lang="postcss">
	.badge {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		text-align: center;
		border-radius: 20px;
		line-height: 90%;

		/* SOLID */

		&.neutral.solid {
			color: var(--clr-scale-ntrl-100);
			background-color: var(--clr-scale-ntrl-40);
		}

		&.pop.solid {
			color: var(--clr-theme-pop-on-element);
			background-color: var(--clr-theme-pop-element);
		}

		&.success.solid {
			color: var(--clr-theme-succ-on-element);
			background-color: var(--clr-theme-succ-element);
		}

		&.warning.solid {
			color: var(--clr-theme-warn-on-element);
			background-color: var(--clr-theme-warn-element);
		}

		&.error.solid {
			color: var(--clr-theme-err-on-element);
			background-color: var(--clr-theme-err-element);
		}

		&.purple.solid {
			color: var(--clr-theme-purp-on-element);
			background-color: var(--clr-theme-purp-element);
		}

		/* SOFT */
		&.neutral.soft {
			color: var(--clr-text-1);
			background-color: var(--clr-scale-ntrl-80);
		}

		&.pop.soft {
			color: var(--clr-theme-pop-on-soft);
			background-color: var(--clr-theme-pop-soft);
		}

		&.success.soft {
			color: var(--clr-theme-succ-on-soft);
			background-color: var(--clr-theme-succ-soft);
		}

		&.warning.soft {
			color: var(--clr-theme-warn-on-soft);
			background-color: var(--clr-theme-warn-soft);
		}

		&.error.soft {
			color: var(--clr-theme-err-on-soft);
			background-color: var(--clr-theme-err-soft);
		}

		&.purple.soft {
			color: var(--clr-theme-purp-on-soft);
			background-color: var(--clr-theme-purp-soft);
		}

		/* SIZE */
		&.icon-size {
			height: var(--size-icon);
			padding: 0 3px;
			gap: 1px;
			min-width: var(--size-icon);
		}

		&.tag-size {
			height: var(--size-tag);
			padding: 0 5px;
			gap: 2px;
			min-width: var(--size-tag);
		}

		/* REVERSED DIRECTION */
		&.reversedDirection {
			flex-direction: row-reverse;
		}
	}

	.badge__label {
		display: flex;
		padding: 0 2px;
		white-space: nowrap;
	}
</style>
