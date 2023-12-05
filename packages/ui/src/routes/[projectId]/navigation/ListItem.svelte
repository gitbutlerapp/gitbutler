<script lang="ts">
	import Icon from '$lib/icons/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';
	import { createEventDispatcher } from 'svelte';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let selected = false;

	const dispatch = createEventDispatcher();
</script>

<button disabled={selected} class="button" class:selected on:click={() => dispatch('click')}>
	<div class="label text-base-14 text-bold">
		<slot />
	</div>
	{#if icon}
		<div class="icon">
			<Icon name={icon} />
		</div>
	{/if}
</button>

<style lang="postcss">
	.button {
		display: flex;
		align-items: center;
		color: var(--clr-theme-scale-ntrl-10);
		font-weight: 700;
		padding: var(--space-10) var(--space-10);
		justify-content: space-between;
		border-radius: var(--radius-m);
		width: 100%;
		&:hover:enabled,
		&:focus:enabled {
			background-color: var(--clr-theme-container-pale);
			& .icon {
				color: var(--clr-theme-scale-ntrl-40);
			}
		}
		&:disabled {
			background-color: var(--clr-theme-container-pale);
			color: var(--clr-theme-scale-ntrl-50);
		}
		& .icon {
			display: flex;
			color: var(--clr-theme-scale-ntrl-50);
		}
		& .label {
			height: var(--space-16);
		}
	}
</style>
