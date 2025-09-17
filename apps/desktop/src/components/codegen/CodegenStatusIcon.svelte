<script lang="ts">
	export let status: 'pending' | 'in_progress' | 'completed';
</script>

<div
	class="status-icon"
	class:pending={status === 'pending'}
	class:completed={status === 'completed'}
>
	{#if status === 'in_progress'}
		<div class="status-icon__hour-hand"></div>
		<div class="status-icon__minute-hand"></div>
	{/if}

	<svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
		{#if status === 'completed'}
			<path
				d="M10 5L7.14329 8.17412C6.74605 8.6155 6.05395 8.6155 5.65671 8.17412L4 6.33333"
				stroke="var(--icon-inner-color)"
				stroke-width="1.5"
			/>
		{/if}
		<circle cx="7" cy="7" r="6.25" stroke="var(--icon-frame-color)" stroke-width="1.5" />
	</svg>
</div>

<style lang="post-css">
	.status-icon {
		display: flex;
		position: relative;
		flex-shrink: 0;
		width: 16px;
		height: 16px;
		transform: translateY(0.156rem);
		--icon-frame-color: color-mix(in srgb, var(--clr-text-1) 30%, transparent);
		--icon-inner-color: color-mix(in srgb, var(--clr-text-1) 80%, transparent);

		&.completed {
			--icon-frame-color: var(--clr-scale-succ-50);
			--icon-inner-color: var(--clr-scale-succ-50);
		}
	}

	.status-icon__hour-hand,
	.status-icon__minute-hand {
		position: absolute;
		left: 6px;
		transform-origin: bottom center;
		border-radius: 1px;
		background-color: black;
	}

	.status-icon__hour-hand {
		top: calc(50%);
		width: 2px;
		height: 5px;
		transform: translateY(-100%);
		background-color: var(--icon-inner-color);
	}

	.status-icon__minute-hand {
		top: calc(50% - 1px);
		width: 2px;
		height: 4px;
		transform: translateY(-100%);
		background-color: var(--icon-inner-color);
		animation: second-hand-spin 2s linear infinite;
	}

	@keyframes second-hand-spin {
		from {
			transform: translateY(-100%) rotate(0deg);
		}
		to {
			transform: translateY(-100%) rotate(360deg);
		}
	}
</style>
