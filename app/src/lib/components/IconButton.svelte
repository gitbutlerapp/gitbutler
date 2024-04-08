<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { tooltip } from '$lib/utils/tooltip';
	import type iconsJson from '$lib/icons/icons.json';

	export let icon: keyof typeof iconsJson;
	export let size: 's' | 'm' | 'l' = 'l';
	export let loading = false;
	export let help = '';
	export let width: string | undefined = undefined;

	let className = '';
	let selected = false;
	export { className as class };
	export let title = '';
</script>

<button
	class="icon-btn {className} size-{size}"
	class:selected
	use:tooltip={help}
	{title}
	on:click
	on:mousedown
	style:width
>
	<Icon name={loading ? 'spinner' : icon} />
</button>

<style lang="postcss">
	.icon-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-m);
		color: var(--clr-scale-ntrl-50);
		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast);
		&:not(.selected):hover {
			background-color: var(--clr-bg-muted);
			color: var(--clr-scale-ntrl-40);
		}
	}
	.selected {
		background-color: var(--clr-bg-muted);
		cursor: default;
	}
	.size-l {
		height: var(--size-control-cta);
		width: var(--size-control-cta);
		padding: var(--size-8);
	}
	.size-m {
		height: var(--size-control-button);
		width: var(--size-control-button);
		padding: var(--size-4);
	}
	.size-s {
		height: var(--size-control-tag);
		width: var(--size-control-tag);
		padding: var(--size-2);
	}
</style>
