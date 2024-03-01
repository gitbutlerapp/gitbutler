<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { tooltip } from '$lib/utils/tooltip';
	import type iconsJson from '$lib/icons/icons.json';

	export let icon: keyof typeof iconsJson;
	export let size: 's' | 'm' | 'l' | 'xl' = 'l';
	export let loading = false;
	export let help = '';
	export let width: string | undefined = undefined;

	let className = '';
	let selected = false;
	export { className as class };
	export let title = '';
</script>

<button
	class="icon-btn {className}"
	class:selected
	class:small={size == 's'}
	class:medium={size == 'm'}
	class:large={size == 'l'}
	class:x-large={size == 'xl'}
	use:tooltip={help}
	{title}
	on:click
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
		color: var(--clr-theme-scale-ntrl-50);
		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast);
		&:not(.selected):hover {
			background-color: color-mix(in srgb, transparent, var(--darken-tint-light));
			color: var(--clr-theme-scale-ntrl-40);
		}
	}
	.selected {
		background-color: color-mix(in srgb, transparent, var(--darken-tint-light));
		cursor: default;
	}
	.x-large {
		height: var(--size-btn-xl);
		width: var(--size-btn-xl);
		padding: var(--space-12);
	}
	.large {
		height: var(--size-btn-l);
		width: var(--size-btn-l);
		padding: var(--space-8);
	}
	.medium {
		height: var(--size-btn-m);
		width: var(--size-btn-m);
		padding: var(--space-4);
	}
	.small {
		height: var(--size-btn-s);
		width: var(--size-btn-s);
		padding: var(--space-2);
	}
</style>
