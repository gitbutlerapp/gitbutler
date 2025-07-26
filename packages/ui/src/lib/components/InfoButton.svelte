<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import { portal } from '$lib/utils/portal';
	import { tooltip } from '$lib/utils/tooltipPosition';
	import { flyScale } from '$lib/utils/transitions';
	import type iconsJson from '$lib/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		title?: string;
		size?: 'small' | 'medium';
		maxWidth?: string;
		icon?: keyof typeof iconsJson;
		inheritColor?: boolean;
		children: Snippet;
	}

	const {
		title,
		size = 'medium',
		maxWidth = '16rem',
		icon,
		children,
		inheritColor
	}: Props = $props();

	let targetEl: HTMLElement | undefined = $state();
	let show = $state(false);
	let timeoutId: undefined | ReturnType<typeof setTimeout> = $state();
	let isHoveringCard = false; // Track if the tooltip card is hovered
	const gapDelay = 150; // Delay to allow transitioning between button and card

	function handleMouseEnter() {
		clearTimeout(timeoutId);
		timeoutId = setTimeout(() => {
			show = true;
		}, 500);
	}

	function handleMouseLeave() {
		clearTimeout(timeoutId);
		timeoutId = setTimeout(() => {
			if (!isHoveringCard) {
				show = false;
			}
		}, gapDelay);
	}

	function handleCardMouseEnter() {
		clearTimeout(timeoutId);
		isHoveringCard = true;
	}

	function handleCardMouseLeave() {
		isHoveringCard = false;
		timeoutId = setTimeout(() => {
			if (!isHoveringCard) {
				show = false;
			}
		}, gapDelay);
	}
</script>

<div
	bind:this={targetEl}
	class="wrapper {size}"
	role="tooltip"
	onmouseenter={handleMouseEnter}
	onmouseleave={handleMouseLeave}
>
	{#if icon}
		<div class="info-custom-icon" class:inherit-color={inheritColor}>
			<Icon name={icon} />
		</div>
	{:else}
		<div class="info-button" class:button-hovered={show}></div>
	{/if}

	{#if show}
		<div
			use:portal={'body'}
			use:tooltip={{
				targetEl,
				position: 'bottom',
				align: 'center'
			}}
			class="tooltip-container"
			role="presentation"
			transition:flyScale
			onmouseenter={handleCardMouseEnter}
			onmouseleave={handleCardMouseLeave}
		>
			<div class="tooltip-card" style:max-width={maxWidth}>
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
		--default-size: 14px;
		--small-size: 12px;

		display: inline-flex;
		position: relative;
		transform: translateY(10%);
	}

	.info-custom-icon {
		display: flex;
		color: var(--clr-text-1);
		transition: all var(--transition-fast);

		&.inherit-color {
			color: inherit;
		}
	}

	.info-button {
		position: relative;
		width: 50px;
		border-radius: var(--default-size);
		box-shadow: inset 0 0 0 1.5px var(--clr-text-2);
		color: var(--clr-text-2);
		transition: box-shadow var(--transition-fast);

		&::before,
		&::after {
			position: absolute;
			left: 50%;
			transform: translateX(-50%);
			border-radius: 2px;
			background-color: var(--clr-text-2);
			content: '';
			transition: background-color var(--transition-fast);
		}
	}

	.button-hovered {
		box-shadow: inset 0 0 0 10px var(--clr-text-2);

		&::before,
		&::after {
			background-color: var(--clr-scale-ntrl-100);
		}
	}

	.wrapper.medium {
		& .info-button {
			width: var(--default-size);
			height: var(--default-size);

			&::before {
				top: 3px;
				width: 2px;
				height: 2px;
			}
			&::after {
				top: 6px;
				width: 2px;
				height: 5px;
			}
		}
	}

	.wrapper.small {
		& .info-button {
			width: var(--small-size);
			height: var(--small-size);

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
	}

	.tooltip-container {
		display: flex;
		z-index: var(--z-blocker);
		position: absolute;
		flex-direction: column;
		width: fit-content;
	}

	.tooltip-card {
		display: flex;
		flex-direction: column;
		width: max-content;
		padding: 12px;
		gap: 4px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-m);
	}

	.tooltip-title {
		color: var(--clr-text-1);
		user-select: text;
	}

	.tooltip-description {
		color: var(--clr-scale-ntrl-40);
		user-select: text;
	}
</style>
