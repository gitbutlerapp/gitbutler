<script lang="ts">
	// import { fly } from 'svelte/transition';
	import { flyScale } from '$lib/utils/transitions';
	import type { Snippet } from 'svelte';

	interface Props {
		title?: string;
		size?: 'small' | 'medium';
		children: Snippet;
	}

	const { title, size = 'medium', children }: Props = $props();

	let show = $state(false);
	let timeoutId: undefined | ReturnType<typeof setTimeout> = $state();

	function handleMouseEnter() {
		timeoutId = setTimeout(() => {
			show = true;
		}, 700);
	}

	function handleMouseLeave() {
		clearTimeout(timeoutId);
		show = false;
	}
</script>

<div class="wrapper" role="tooltip" onmouseenter={handleMouseEnter} onmouseleave={handleMouseLeave}>
	<div class="info-button {size}"></div>

	{#if show}
		<div class="tooltip-container" transition:flyScale>
			<div class="tooltip-arrow"></div>

			<div class="tooltip-card">
				{#if title}
					<h3 class="text-13 text-semibold tooltip-title">{title}</h3>
				{/if}
				<p class="text-12 text-body tooltip-description">
					{@render children()}
				</p>
			</div>
		</div>
	{/if}
</div>

<style lang="postcss">
	.wrapper {
		position: relative;
		display: inline-flex;
	}

	.info-button {
		position: relative;
		flex-shrink: 0;
		color: var(--clr-text-2);
		border-radius: 16px;
		box-shadow: inset 0 0 0 1.5px var(--clr-text-2);

		&::before,
		&::after {
			content: '';
			position: absolute;
			left: 50%;
			transform: translateX(-50%);
			background-color: var(--clr-text-2);
			border-radius: 2px;
		}
	}

	.info-button.medium {
		width: 16px;
		height: 16px;

		&::before {
			top: 4px;
			width: 2px;
			height: 2px;
		}

		&::after {
			top: 7px;
			width: 2px;
			height: 5px;
		}
	}

	.info-button.small {
		width: 12px;
		height: 12px;

		&::before {
			top: 3px;
			width: 2px;
			height: 2px;
		}

		&::after {
			top: 6px;
			width: 2px;
			height: 3px;
		}
	}

	.tooltip-container {
		z-index: var(--z-blocker);
		position: absolute;
		top: 100%;
		left: 50%;
		transform: translateX(-50%);
		display: flex;
		flex-direction: column;
	}

	.tooltip-card {
		display: flex;
		flex-direction: column;
		gap: 4px;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		padding: 12px;
		width: max-content;
		max-width: 260px;
		box-shadow: var(--fx-shadow-m);
	}

	.tooltip-title {
		color: var(--clr-text-1);
	}

	.tooltip-description {
		color: var(--clr-scale-ntrl-40);
	}

	.tooltip-arrow {
		position: relative;
		top: 1px;
		margin: 0 auto;
		width: 100%;
		height: 10px;
		display: flex;
		justify-content: center;
		overflow: hidden;
		z-index: var(--z-lifted);
		width: fit-content;

		&::before {
			content: '';
			position: relative;
			top: 4px;
			width: 20px;
			height: 20px;
			transform: rotate(45deg);
			border-radius: 2px;
			background-color: var(--clr-bg-1);
			border: 1px solid var(--clr-border-2);
		}
	}
</style>
