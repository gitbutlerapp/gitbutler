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
	width="16"
	height="16"
	viewBox="0 0 16 16"
	fill-rule="evenodd"
	class:success={color == 'success'}
	class:error={color == 'error'}
	class:pop={color == 'pop'}
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
</style>
