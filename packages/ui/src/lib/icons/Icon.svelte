<script lang="ts" context="module">
	export type IconColor = 'success' | 'error' | 'pop' | undefined;
	export type jsonIcon = {
		size: number;
		path: string;
	};
</script>

<script lang="ts">
	import iconsJson from './icons.json';

	let className = '';
	export { className as class };
	export let name: keyof typeof iconsJson;
	export let color: IconColor = undefined;

	$: svg = iconsJson[name] as jsonIcon;
</script>

<svg
	class={className}
	style:width={svg.size}
	style:height={svg.size}
	viewBox={`0 0 ${svg.size} ${svg.size}`}
	fill-rule="evenodd"
	class:success={color == 'success'}
	class:error={color == 'error'}
	class:pop={color == 'pop'}
>
	<path fill="currentColor" d={svg.path}></path>
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
