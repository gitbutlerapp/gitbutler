<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { createEventDispatcher } from 'svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let id: string | undefined = undefined; // Required to make label clickable
	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let value: string | undefined = undefined;
	export let placeholder: string | undefined = undefined;
	export let iconPosition: 'left' | 'right' = 'left';
	export let label: string | undefined = undefined;
	export let disabled = false;
	export let readonly = false;
	export let required = false;
	export let password = false;
	export let noselect = false;
	export let selectall = false;
	export let element: HTMLElement | undefined = undefined;
	export let spellcheck = false;

	const dispatch = createEventDispatcher<{ input: string; change: string }>();
</script>

<div class="textbox" bind:this={element}>
	{#if label}
		<label class="textbox__label font-base-13 text-semibold" for={id}>
			{label}
		</label>
	{/if}
	<div
		class="textbox__input-wrap"
		class:textbox__left-orient={icon && iconPosition == 'left'}
		class:textbox__right-orient={icon && iconPosition == 'right'}
		class:disabled
	>
		{#if icon}
			<div class="textbox__icon">
				<Icon name={icon} />
			</div>
		{/if}
		{#if password}
			<input
				{id}
				{readonly}
				{required}
				{placeholder}
				{spellcheck}
				{disabled}
				type="password"
				class="text-input textbox__input text-base-13"
				class:select-none={noselect}
				bind:value
				on:click
				on:input={(e) => dispatch('input', e.currentTarget.value)}
				on:change={(e) => dispatch('change', e.currentTarget.value)}
			/>
		{:else}
			<input
				{id}
				{readonly}
				{required}
				{placeholder}
				{spellcheck}
				{disabled}
				class="text-input textbox__input text-base-13"
				class:select-none={noselect}
				class:select-all={selectall}
				bind:value
				on:click
				on:input={(e) => dispatch('input', e.currentTarget.value)}
				on:change={(e) => dispatch('change', e.currentTarget.value)}
			/>
		{/if}
	</div>
</div>

<style lang="postcss">
	.textbox {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}

	.textbox__input-wrap {
		position: relative;
	}

	.textbox__input {
		flex-grow: 1;
		height: var(--size-btn-l);
		width: 100%;

		&:disabled {
			& .textbox__icon {
				color: var(--clr-theme-scale-ntrl-60);
			}
		}
	}

	.textbox__label {
		color: var(--clr-theme-scale-ntrl-50);
	}

	.textbox__icon {
		pointer-events: none;
		position: absolute;
		top: 50%;
		color: var(--clr-theme-scale-ntrl-50);
		transform: translateY(-50%);
	}

	/* Modifiers */

	.textbox__left-orient {
		& .textbox__input {
			padding-left: calc(var(--space-32) + var(--space-2));
		}
		& .textbox__icon {
			left: var(--space-12);
		}
	}

	.textbox__right-orient {
		& .textbox__input {
			padding-right: calc(var(--space-32) + var(--space-2));
		}
		& .textbox__icon {
			right: var(--space-12);
		}
	}
</style>
