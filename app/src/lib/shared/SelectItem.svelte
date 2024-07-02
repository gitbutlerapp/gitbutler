<script lang="ts">
	import Icon from '$lib/shared/Icon.svelte';
	import { createEventDispatcher } from 'svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let selected = false;
	export let disabled = false;
	export let loading = false;
	export let highlighted = false;
	export let value: string | undefined = undefined;

	const dispatch = createEventDispatcher<{ click: string | undefined }>();
</script>

<button
	{disabled}
	class="button"
	class:selected
	class:highlighted
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
		color: var(--clr-scale-ntrl-10);
		font-weight: 700;
		padding: 8px 8px;
		justify-content: space-between;
		border-radius: var(--radius-m);
		width: 100%;
		white-space: nowrap;
		&:not(.selected):hover:enabled,
		&:not(.selected):focus:enabled {
			background-color: var(--clr-bg-1-muted);
			& .icon {
				color: var(--clr-scale-ntrl-40);
			}
		}
		&:disabled {
			background-color: var(--clr-bg-2);
			color: var(--clr-scale-ntrl-50);
		}
		& .icon {
			display: flex;
			color: var(--clr-scale-ntrl-50);
		}
		& .label {
			height: 16px;
			text-overflow: ellipsis;
			overflow-x: hidden;
		}
	}

	.selected {
		background-color: var(--clr-bg-2);

		& .label {
			opacity: 0.5;
		}
	}

	.highlighted {
		background-color: var(--clr-bg-3);
	}
</style>
