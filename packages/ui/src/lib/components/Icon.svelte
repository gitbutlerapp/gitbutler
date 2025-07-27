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
		color?: IconColor;
		opacity?: number | undefined;
		spinnerRadius?: number | undefined;
		size?: number;
		rotate?: number | undefined;
		verticalAlign?: string;
	}

	const {
		name,
		color = undefined,
		opacity = 1,
		spinnerRadius = 5,
		size = 16,
		rotate = undefined,
		verticalAlign
	}: Props = $props();
</script>

<svg
	class="icon-wrapper"
	class:success={color === 'success'}
	class:error={color === 'error'}
	class:pop={color === 'pop'}
	class:warning={color === 'warning'}
	viewBox="0 0 16 16"
	fill-rule="evenodd"
	class:default={!color}
	style:fill-opacity={opacity}
	style:width="{pxToRem(size)}rem"
	style:height="{pxToRem(size)}rem"
	style:transform={rotate ? `rotate(${rotate}deg)` : undefined}
	style:vertical-align={verticalAlign}
	style="--spinner-radius: {spinnerRadius}"
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
		color: var(--clr-scale-succ-50);
	}
	.error {
		color: var(--clr-scale-err-50);
	}
	.pop {
		color: var(--clr-scale-pop-50);
	}
	.warning {
		color: var(--clr-scale-warn-50);
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
