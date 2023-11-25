<script lang="ts">
	import type iconsJson from '$lib/icons/icons.json';
	import Icon from '$lib/icons/Icon.svelte';
	import { createEventDispatcher } from 'svelte';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let iconPosition: 'left' | 'right' = 'left';

	const dispatch = createEventDispatcher<{ input: string }>();
</script>

<div class="textbox">
	{#if icon && iconPosition == 'left'}
		<Icon name={icon} />
	{/if}
	<input
		type="text text-base-13"
		class="textbox__input"
		on:input={(e) => dispatch('input', e.currentTarget.value)}
	/>
	{#if icon && iconPosition == 'right'}
		<Icon name={icon} />
	{/if}
</div>

<style lang="postcss">
	.textbox {
		display: flex;
		width: 100%;
		color: var(--clr-theme-scale-ntrl-50);
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		border-radius: var(--radius-s);
		align-items: center;
		gap: var(--space-8);
		padding-top: var(--space-4);
		padding-bottom: var(--space-4);
		padding-left: var(--space-12);
		padding-right: var(--space-12);

		&:hover {
			border-color: var(--clr-theme-container-outline-pale);
		}
		&:focus {
			border-color: var(--clr-theme-container-outline-sub);
		}
		&:invalid {
			border-color: var(--clr-theme-err-element);
		}
		&:disabled {
			color: var(--clr-theme-scale-ntrl-60);
			border-color: var(--clr-theme-err-element);
			background-color: var(--clr-theme-container-pale);
		}
	}
	.textbox__input {
		flex-grow: 1;
	}
</style>
