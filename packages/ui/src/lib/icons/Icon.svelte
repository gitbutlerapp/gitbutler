<script lang="ts" context="module">
	export type IconColor = 'success' | 'error' | 'pop' | undefined;
</script>

<script lang="ts">
	import iconsJson from './icons.json';

	let className = '';
	export { className as class };
	export let name: keyof typeof iconsJson;
	export let color: IconColor = undefined;
</script>

<svg
	class={className}
	class:spinning={name == 'spinner'}
	width="16"
	height="16"
	style="width: 1rem; height: 1rem;"
	viewBox="0 0 16 16"
	fill-rule="evenodd"
	class:success={color == 'success'}
	class:error={color == 'error'}
	class:pop={color == 'pop'}
	class:default={!color}
>
	<path fill="currentColor" d={iconsJson[name]}></path>
</svg>

<style>
	.success {
		color: var(--clr-core-succ-40);
	}
	.error {
		color: var(--clr-core-err-40);
	}
	.pop {
		color: var(--clr-core-pop-40);
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

	svg {
		pointer-events: none;
		display: inline-block;
	}
</style>
