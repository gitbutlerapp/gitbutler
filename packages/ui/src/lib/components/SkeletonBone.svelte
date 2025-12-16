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
		color = 'var(--clr-border-2)',
		opacity = 0.34
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
			opacity: calc(var(--opacity-value) + var(--opacity-value) * 0.7);
		}
	}
</style>
