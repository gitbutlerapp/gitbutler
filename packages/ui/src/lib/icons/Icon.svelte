<script lang="ts" context="module">
	export type IconColor = 'success' | 'error' | 'pop' | 'warn' | undefined;
</script>

<script lang="ts">
	import iconsJson from './icons.json';

	export let name: keyof typeof iconsJson;
	export let color: IconColor = undefined;
	export let opacity: string | undefined = undefined;
</script>

<svg
	class="icon-wrapper"
	class:spinning={name == 'spinner'}
	viewBox="0 0 16 16"
	fill-rule="evenodd"
	class:success={color == 'success'}
	class:error={color == 'error'}
	class:pop={color == 'pop'}
	class:warn={color == 'warn'}
	class:default={!color}
	style:opacity
>
	<path fill="currentColor" d={iconsJson[name]}></path>
</svg>

<style lang="postcss">
	.icon-wrapper {
		width: 1rem;
		height: 1rem;
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
		color: var(--clr-theme-scale-warn-60);
	}

	.spinning {
		transform-origin: center;
		animation: spinning 0.75s infinite linear;
	}
	@keyframes spinning {
		100% {
			transform: rotate(360deg);
		}
	}
</style>
