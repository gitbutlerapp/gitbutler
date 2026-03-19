<script lang="ts">
	import { ActiveOperationIndicator } from "$lib/activeOperations.svelte";
	import { onDestroy } from "svelte";

	const indicator = new ActiveOperationIndicator();
	onDestroy(() => indicator.destroy());
</script>

{#if indicator.visible}
	<div class="working-indicator" role="status" aria-label="Operation in progress">
		<div class="working-indicator__dot"></div>
	</div>
{/if}

<style lang="postcss">
	.working-indicator {
		z-index: var(--z-floating);
		position: fixed;
		bottom: 8px;
		right: 8px;
		display: flex;
		align-items: center;
		justify-content: center;
		pointer-events: none;
	}

	.working-indicator__dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background-color: var(--clr-theme-pop-element);
		animation: pulse 1s ease-in-out infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
			transform: scale(1);
		}
		50% {
			opacity: 0.4;
			transform: scale(0.75);
		}
	}
</style>
