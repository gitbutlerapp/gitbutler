<script lang="ts" context="module">
	import { pxToRem } from '$lib/utils/pxToRem';
	export type IconColor = 'success' | 'error' | 'pop' | 'warn' | 'neutral' | undefined;
</script>

<script lang="ts">
	import iconsJson from '../icons/icons.json';

	export let name: keyof typeof iconsJson;
	export let color: IconColor = undefined;
	export let opacity: number | undefined = 1;
	export let spinnerRadius: number | undefined = 5;
	export let size = 16;
</script>

<svg
	class="icon-wrapper"
	viewBox="0 0 16 16"
	fill-rule="evenodd"
	class:spinner={name == 'spinner'}
	class:success={color == 'success'}
	class:error={color == 'error'}
	class:pop={color == 'pop'}
	class:warn={color == 'warn'}
	class:default={!color}
	style:fill-opacity={opacity}
	style:width={pxToRem(size)}
	style:height={pxToRem(size)}
>
	{#if name == 'spinner'}
		<circle class="spinner-path" cx="8" cy="8" r={spinnerRadius} fill="none" />
	{:else}
		<path fill="currentColor" d={iconsJson[name]} />
	{/if}
</svg>

<style lang="postcss">
	.icon-wrapper {
		flex-shrink: 0;
		pointer-events: none;
		display: inline-block;
	}

	.success {
		color: var(--clr-theme-scale-succ-50);
	}
	.error {
		color: var(--clr-theme-scale-err-50);
	}
	.pop {
		color: var(--clr-theme-scale-pop-50);
	}
	.warn {
		color: var(--clr-theme-scale-warn-50);
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
		stroke-width: calc(var(--space-2) / 1.4);
		stroke: currentColor;
		animation: spinning-path 1.5s infinite ease-in-out;
	}
	@keyframes spinning-path {
		0% {
			stroke-dasharray: 1, 200;
			stroke-dashoffset: 0;
		}
		50% {
			stroke-dasharray: 60, 200;
			stroke-dashoffset: -12;
		}
		100% {
			stroke-dasharray: 60, 200;
			stroke-dashoffset: -34;
		}
	}
</style>
