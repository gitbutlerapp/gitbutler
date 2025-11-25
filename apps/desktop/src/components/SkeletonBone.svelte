<script lang="ts">
	interface Props {
		width?: string;
		height?: string;
		radius?: string;
		color?: string;
		opacity?: number;
	}

	const {
		width = '100%',
		height = '1rem',
		radius = 'var(--radius-ml)',
		color = 'var(--clr-text-3)',
		opacity = 0.14
	}: Props = $props();
</script>

<div
	style:width
	style:height
	style:border-radius={radius}
	style:background-color={color}
	style:--opacity-value={opacity}
	class="skeleton-bone"
></div>

<style lang="postcss">
	.skeleton-bone {
		animation: pulse 0.8s ease-in-out infinite alternate;
	}

	@keyframes pulse {
		0% {
			opacity: var(--opacity-value);
		}
		100% {
			opacity: calc(var(--opacity-value) + 0.1);
		}
	}
</style>
