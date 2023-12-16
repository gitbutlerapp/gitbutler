<script lang="ts">
	import Icon from '$lib/icons/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let icon: keyof typeof iconsJson;
	export let size: 's' | 'm' | 'l' = 'l';
	export let loading = false;

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
	{title}
	on:click
>
	<Icon name={loading ? 'spinner' : icon} />
</button>

<!-- {className} text-color-4 hover:text-color-1 outline-blue-200 focus:text-light-800 focus:outline-2 disabled:cursor-not-allowed
disabled:text-light-200 dark:text-dark-400 hover:dark:text-dark-100 focus:dark:text-dark-600 dark:disabled:text-dark-400 -->

<style lang="postcss">
	.icon-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		color: var(--clr-theme-scale-ntrl-40);
		border-radius: var(--radius-m);
		color: var(--clr-theme-scale-ntrl-50);
		&:not(.selected):hover {
			background-color: var(--clr-theme-container-pale);
			color: var(--clr-theme-scale-ntrl-40);
		}
	}
	.selected {
		background-color: var(--clr-theme-container-sub);
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
