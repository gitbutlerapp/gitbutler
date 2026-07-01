<script lang="ts">
	import Icon from "$components/Icon.svelte";
	import SkeletonBone from "$components/SkeletonBone.svelte";
	import Tooltip from "$components/Tooltip.svelte";
	import { type IconName } from "$lib/icons/names";
	import type { ComponentColorType } from "$lib/utils/colorTypes";
	import type { Snippet } from "svelte";

	interface Props {
		testId?: string;
		style?: ComponentColorType;
		kind?: "solid" | "soft";
		size?: "icon" | "tag";
		class?: string;
		icon?: IconName;
		tooltip?: string;
		skeleton?: boolean;
		skeletonWidth?: string;
		children?: Snippet;
		onclick?: (e: MouseEvent) => void;
		reversedDirection?: boolean;
	}

	const {
		testId,
		style = "gray",
		kind = "solid",
		size = "icon",
		class: className = "",
		icon,
		tooltip,
		skeleton,
		skeletonWidth,
		children,
		onclick,
		reversedDirection,
	}: Props = $props();
</script>

{#if skeleton}
	<SkeletonBone
		radius="3rem"
		width={skeletonWidth ?? (size === "icon" ? "var(--size-icon)" : "var(--size-tag)")}
		height={size === "icon" ? "var(--size-icon)" : "var(--size-tag)"}
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
				<span class="badge__label text-11 text-bold">{@render children()}</span>
			{/if}
			{#if icon}
				<i class="badge__icon">
					<Icon name={icon} size={11} />
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
		border-radius: 30px;
		text-align: center;

		/* SOLID */
		&.gray.solid {
			background-color: var(--fill-gray-bg);
			color: var(--fill-gray-fg);
		}

		&.pop.solid {
			background-color: var(--fill-pop-bg);
			color: var(--fill-pop-fg);
		}

		&.safe.solid {
			background-color: var(--fill-safe-bg);
			color: var(--fill-safe-fg);
		}

		&.warning.solid {
			background-color: var(--fill-warn-bg);
			color: var(--fill-warn-fg);
		}

		&.danger.solid {
			background-color: var(--fill-danger-bg);
			color: var(--fill-danger-fg);
		}

		&.purple.solid {
			background-color: var(--fill-purple-bg);
			color: var(--fill-purple-fg);
		}

		/* SOFT */
		&.gray.soft {
			background-color: var(--chip-gray-bg);
			color: var(--chip-gray-fg);
		}

		&.pop.soft {
			background-color: var(--chip-pop-bg);
			color: var(--chip-pop-fg);
		}

		&.safe.soft {
			background-color: var(--chip-safe-bg);
			color: var(--chip-safe-fg);
		}

		&.warning.soft {
			background-color: var(--chip-warn-bg);
			color: var(--chip-warn-fg);
		}

		&.danger.soft {
			background-color: var(--chip-danger-bg);
			color: var(--chip-danger-fg);
		}

		&.purple.soft {
			background-color: var(--chip-purple-bg);
			color: var(--chip-purple-fg);
		}

		/* SIZE */
		&.icon-size {
			padding: 3px 6px;
			gap: 3px;
		}

		&.tag-size {
			height: var(--size-tag);
			padding: 3px 8px;
			gap: 3px;
		}

		&.reversed {
			flex-direction: row-reverse;
		}
	}

	.badge__label {
		display: flex;
		line-height: var(--size-icon);
		line-height: 1;
		white-space: nowrap;
	}

	.badge__icon {
		display: flex;
		opacity: 0.7;
	}

	@supports (text-box: trim-both ex alphabetic) {
		.badge__label {
			text-box: trim-both ex alphabetic;
		}
	}

	@support not (text-box: trim-both ex alphabetic) {
		.badge__label {
			padding-top: 1px;
		}
	}
</style>
