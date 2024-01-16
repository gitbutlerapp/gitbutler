<script lang="ts">
	import Icon from '$lib/icons/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';
	import { createEventDispatcher } from 'svelte';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let selected = false;
	export let disabled = false;
	export let loading = false;
	export let value: string | undefined = undefined;

	const dispatch = createEventDispatcher<{ click: string | undefined }>();
</script>

<button
	disabled={selected || disabled}
	class="button"
	class:selected
	on:click={() => dispatch('click', value)}
>
	<div class="label text-base-13">
		<slot />
	</div>
	{#if icon || selected}
		<div class="icon">
			{#if icon}
				<Icon name={loading ? 'spinner' : icon} />
			{:else}
				<Icon name="tick" />
			{/if}
		</div>
	{/if}
</button>

<style lang="postcss">
	.button {
		display: flex;
		align-items: center;
		color: var(--clr-theme-scale-ntrl-10);
		font-weight: 700;
		padding: var(--space-8) var(--space-8);
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
			text-overflow: ellipsis;
			overflow-x: hidden;
		}
	}
</style>
