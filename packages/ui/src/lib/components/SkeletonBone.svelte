<script lang="ts">
	interface Props {
		width?: string;
		maxWidth?: string;
		height?: string;
		radius?: string;
		color?: string;
		opacity?: number;
	}

	const {
		width = "100%",
		maxWidth,
		height = "1rem",
		radius = "var(--radius-ml)",
		color = "var(--border-2)",
		opacity = 0.2,
	}: Props = $props();
</script>

<div
	style:width
	style:height
	style:max-width={maxWidth}
	style:border-radius={radius}
	style:background-color={color}
	style:--opacity-value={opacity}
	class="skeleton-bone"
></div>

<style lang="postcss">
	.skeleton-bone {
		position: relative;
		overflow: hidden;
		animation: skeleton-pulse var(--motion-duration-loading) var(--motion-ease-standard) infinite
			alternate;
	}

	.skeleton-bone::after {
		position: absolute;
		inset: 0;
		background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.18), transparent);
		content: "";
		animation: skeleton-shimmer calc(var(--motion-duration-loading) * 1.6)
			var(--motion-ease-decelerate) infinite;
	}

	@keyframes skeleton-pulse {
		0% {
			opacity: var(--opacity-value);
		}
		100% {
			opacity: calc(var(--opacity-value) + var(--opacity-value) * 0.7);
		}
	}

	@keyframes skeleton-shimmer {
		from {
			transform: translateX(-100%);
		}
		to {
			transform: translateX(100%);
		}
	}
</style>
