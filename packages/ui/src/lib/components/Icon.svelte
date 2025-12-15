<script lang="ts" module>
	import iconsJson from '$lib/data/icons.json';
	import { pxToRem } from '$lib/utils/pxToRem';

	export type IconColor = ComponentColorType | undefined;
	export type IconName = keyof typeof iconsJson;
</script>

<script lang="ts">
	import type { ComponentColorType } from '$lib/utils/colorTypes';

	interface Props {
		name: IconName;
		color?: IconColor | string;
		opacity?: number;
		spinnerRadius?: number;
		size?: number;
		rotate?: number;
		verticalAlign?: string;
		noEvents?: boolean;
		zIndex?: string;
	}

	const {
		name,
		color,
		opacity = 1,
		spinnerRadius = 5,
		size = 16,
		rotate,
		verticalAlign,
		noEvents,
		zIndex
	}: Props = $props();

	// Check if color is a predefined type or custom color
	const predefinedColors = ['success', 'error', 'pop', 'warning'];
	const isPredefinedColor = color && predefinedColors.includes(color as string);
	const customColor = !isPredefinedColor ? color : undefined;
</script>

<svg
	viewBox="0 0 16 16"
	fill-rule="evenodd"
	class="icon-wrapper"
	class:success={color === 'success'}
	class:error={color === 'error'}
	class:pop={color === 'pop'}
	class:warning={color === 'warning'}
	class:custom={customColor}
	class:default={!color}
	style:fill-opacity={opacity}
	style:width="{pxToRem(size)}rem"
	style:height="{pxToRem(size)}rem"
	style:transform={rotate ? `rotate(${rotate}deg)` : undefined}
	style:vertical-align={verticalAlign}
	style:z-index={zIndex}
	style:pointer-events={noEvents ? 'none' : undefined}
	style="--spinner-radius: {spinnerRadius}; --custom-color: {customColor || 'currentColor'}"
>
	{#if name === 'spinner'}
		<g class:spinner={name === 'spinner'}>
			<circle class="spinner-path" cx="8" cy="8" r={spinnerRadius} fill="none" />
			<circle
				class="spinner-back-path"
				cx="8"
				cy="8"
				r={spinnerRadius}
				fill="none"
				vector-effect="non-scaling-stroke"
			/>
		</g>
	{:else}
		<path fill="currentColor" d={iconsJson[name]} />
	{/if}
</svg>

<style lang="postcss">
	.icon-wrapper {
		--spinner-stroke-width: 1.5;
		display: inline-block;
		flex-shrink: 0;
	}

	.success {
		color: var(--clr-theme-succ-element);
	}
	.error {
		color: var(--clr-theme-err-element);
	}
	.pop {
		color: var(--clr-theme-pop-element);
	}
	.warning {
		color: var(--clr-theme-warn-element);
	}
	.custom {
		color: var(--custom-color);
	}

	.spinner {
		transform-origin: center;
		animation: spinning 1s infinite linear;
	}
	@keyframes spinning {
		100% {
			transform: rotate(360deg);
		}
	}
	.spinner-path {
		stroke-width: var(--spinner-stroke-width);
		stroke: currentColor;
		animation: spinning-path 2s infinite ease-in-out;
	}

	.spinner-back-path {
		stroke-width: var(--spinner-stroke-width);
		stroke: currentColor;
		opacity: 0.3;
	}
	@keyframes spinning-path {
		0% {
			stroke-dasharray: 1, 120;
			stroke-dashoffset: 0;
		}
		60% {
			stroke-dasharray: 60, 120;
			stroke-dashoffset: -10;
		}
		100% {
			stroke-dasharray: 60, 120;
			stroke-dashoffset: calc(-1 * var(--spinner-radius) * 5.5);
		}
	}
</style>
