<script lang="ts">
	import Tooltip, { type TooltipAlign, type TooltipPosition } from '$components/Tooltip.svelte';
	import { stringToColor } from '$lib/utils/stringToColor';

	interface Props {
		srcUrl: string | null;
		username: string;
		tooltip?: string;
		tooltipAlign?: TooltipAlign;
		tooltipPosition?: TooltipPosition;
		size?: 'small' | 'medium' | 'large';
	}

	const {
		srcUrl,
		username = '',
		tooltip,
		tooltipAlign,
		tooltipPosition,
		size = 'small'
	}: Props = $props();
</script>

<Tooltip text={tooltip ?? username} align={tooltipAlign} position={tooltipPosition}>
	<div
		class="image-wrapper {size}"
		style:background-color={stringToColor(username || srcUrl || undefined)}
	>
		{#if srcUrl}
			<img class="avatar" alt={tooltip} src={srcUrl} loading="lazy" />
		{/if}
	</div>
</Tooltip>

<style lang="postcss">
	.image-wrapper {
		display: grid;
		flex-shrink: 0;
		place-content: center;
		width: 12px;
		height: 12px;
		overflow: hidden;
		border-radius: 6px;
		color: var(--clr-theme-pop-on-element);

		&.small {
			width: 12px;
			height: 12px;
			border-radius: 6px;
		}

		&.medium {
			width: 16px;
			height: 16px;
			border-radius: 8px;
		}

		&.large {
			width: 28px;
			height: 28px;
			border-radius: 16px;
		}
	}

	.image-wrapper > * {
		grid-area: 1 / 1;
	}

	.avatar {
		position: relative;
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
</style>
