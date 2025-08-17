<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import Tooltip from '$components/Tooltip.svelte';
	import type iconsJson from '$lib/data/icons.json';
	import type { ComponentColorType } from '$lib/utils/colorTypes';
	import type { Snippet } from 'svelte';

	interface Props {
		testId?: string;
		style?: ComponentColorType;
		kind?: 'solid' | 'soft';
		size?: 'icon' | 'tag';
		icon?: keyof typeof iconsJson | undefined;
		reversedDirection?: boolean;
		tooltip?: string;
		children?: Snippet;
		onclick?: (e: MouseEvent) => void;
	}

	const {
		testId,
		style = 'neutral',
		kind = 'solid',
		size = 'icon',
		icon,
		reversedDirection,
		tooltip,
		children,
		onclick
	}: Props = $props();
</script>

<Tooltip text={tooltip}>
	<!-- A badge is not a clickable UI element, but with exceptions. No button styling desired. -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div
		data-testid={testId}
		class="badge {style} {kind} {size}-size"
		class:reversedDirection
		{onclick}
	>
		{#if children}
			<span
				class="badge__label text-bold"
				class:text-10={size === 'icon'}
				class:text-11={size === 'tag'}>{@render children()}</span
			>
		{/if}
		{#if icon}
			<i class="badge__icon">
				<Icon name={icon} />
			</i>
		{/if}
	</div>
</Tooltip>

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
		&.neutral.solid {
			background-color: var(--clr-scale-ntrl-40);
			color: var(--clr-scale-ntrl-100);
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
			background-color: var(--clr-theme-err-element);
			color: var(--clr-theme-err-on-element);
		}

		&.purple.solid {
			background-color: var(--clr-theme-purp-element);
			color: var(--clr-theme-purp-on-element);
		}

		/* SOFT */
		&.neutral.soft {
			background-color: var(--clr-scale-ntrl-80);
			color: var(--clr-text-1);
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
			background-color: var(--clr-theme-err-soft);
			color: var(--clr-theme-err-on-soft);
		}

		&.purple.soft {
			background-color: var(--clr-theme-purp-soft);
			color: var(--clr-theme-purp-on-soft);
		}

		/* SIZE */
		&.icon-size {
			min-width: var(--size-icon);
			height: var(--size-icon);
			padding: 0 2px;
			gap: 1px;
		}

		&.tag-size {
			min-width: var(--size-tag);
			height: var(--size-tag);
			padding: 0 4px;
			gap: 2px;
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

	.badge__icon {
		opacity: 0.7;
	}
</style>
