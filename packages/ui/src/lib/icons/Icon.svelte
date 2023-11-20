<script lang="ts">
	import iconsJson from './icons.json';

	let className = '';
	export { className as class };
	export let name: keyof typeof iconsJson;
</script>

<svg class={className} width="16" height="16" viewBox="0 0 16 16" fill-rule="evenodd">
	<path fill="currentColor" d={iconsJson[name]}></path>
</svg>
