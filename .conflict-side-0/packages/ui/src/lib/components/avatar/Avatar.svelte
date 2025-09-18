<script lang="ts">
	import Tooltip, { type TooltipAlign, type TooltipPosition } from '$components/Tooltip.svelte';
	import { stringToColor } from '$lib/utils/stringToColor';

	interface Props {
		srcUrl: string;
		tooltip: string;
		tooltipAlign?: TooltipAlign;
		tooltipPosition?: TooltipPosition;
		size?: 'small' | 'medium' | 'large';
	}

	let isLoaded = $state(false);

	const { srcUrl, tooltip, tooltipAlign, tooltipPosition, size = 'small' }: Props = $props();
</script>

<Tooltip text={tooltip} align={tooltipAlign} position={tooltipPosition}>
	<div class="image-wrapper {size}" style:background-color={stringToColor(srcUrl)}>
		<img
			class="avatar"
			alt={tooltip}
			src={srcUrl}
			loading="lazy"
			onload={() => (isLoaded = true)}
			class:show={isLoaded}
		/>
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
		opacity: 0;
	}

	.show {
		opacity: 1;
	}
</style>
