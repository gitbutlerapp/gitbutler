<script lang="ts">
	import Icon from '$lib/icons/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';
	import { createEventDispatcher } from 'svelte';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let selected = false;

	const dispatch = createEventDispatcher();
</script>

<button
	disabled={selected}
	class="button text-base-14 font-bold"
	class:selected
	on:click={() => dispatch('click')}
>
	<slot />
	{#if icon}
		<div class="button__icon">
			<Icon name={icon} />
		</div>
	{/if}
</button>

<style lang="postcss">
	.button {
		color: var(--clr-theme-scale-ntrl-10);
		font-weight: 700;
		padding-top: var(--space-8);
		padding-bottom: var(--space-8);
		padding-left: var(--space-10);
		padding-right: var(--space-10);
		justify-content: space-between;
		width: 100%;
		&:hover:enabled,
		&:focus:enabled {
			background-color: var(--clr-theme-container-pale);
			& .button__icon {
				color: var(--clr-theme-scale-ntrl-40);
			}
		}
		&:disabled {
			background-color: var(--clr-theme-container-pale);
			color: var(--clr-theme-scale-ntrl-50);
		}
	}
	.button__icon {
		color: var(--clr-theme-scale-ntrl-50);
	}
</style>
