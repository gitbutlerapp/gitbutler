<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { createEventDispatcher, onMount } from 'svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let element: HTMLElement | undefined = undefined;
	export let id: string | undefined = undefined; // Required to make label clickable
	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let value: string | undefined = undefined;
	export let width: number | undefined = undefined;
	export let textAlign: 'left' | 'center' | 'right' = 'left';
	export let placeholder: string | undefined = undefined;
	export let label: string | undefined = undefined;
	export let reversedDirection: boolean = false;
	export let wide: boolean = false;
	export let minVal: number | undefined = undefined;
	export let maxVal: number | undefined = undefined;
	export let disabled = false;
	export let readonly = false;
	export let required = false;
	export let noselect = false;
	export let selectall = false;
	export let spellcheck = false;
	export let focus = false;

	export let type: 'text' | 'password' | 'select' | 'number' = 'text';

	const dispatch = createEventDispatcher<{
		input: string;
		change: string;
		keydown: KeyboardEvent;
	}>();

	let showPassword = false;
	let isInputValid = true;
	let htmlInput: HTMLInputElement;

	onMount(() => {
		if (focus) htmlInput.focus();
	});
</script>

<div
	class="textbox"
	bind:this={element}
	class:wide
	style:width={pxToRem(width)}
	class:shaky={!isInputValid}
>
	{#if label}
		<label class="textbox__label text-base-13 text-semibold" for={id}>
			{label}
		</label>
	{/if}
	<div
		class="textbox__input-wrap"
		class:textbox__left-orient={icon && !reversedDirection}
		class:textbox__right-orient={icon && reversedDirection}
		class:disabled
	>
		{#if icon}
			<div class="textbox__icon">
				<Icon name={icon} />
			</div>
		{/if}

		{#if type == 'number'}
			<div class="textbox__count-actions">
				<button
					class="textbox__count-btn"
					on:click={() => {
						htmlInput.stepDown();
						dispatch('input', htmlInput.value);

						isInputValid = htmlInput.checkValidity();
					}}
				>
					<Icon name="minus-small" />
				</button>
				<button
					class="textbox__count-btn"
					on:click={() => {
						htmlInput.stepUp();
						dispatch('input', htmlInput.value);

						isInputValid = htmlInput.checkValidity();
					}}
				>
					<Icon name="plus-small" />
				</button>
			</div>
		{/if}

		<input
			{id}
			{readonly}
			{required}
			{placeholder}
			{spellcheck}
			{disabled}
			min={minVal}
			max={maxVal}
			{...type == 'password' && showPassword ? { type: 'text' } : { type }}
			class="text-input textbox__input text-base-13"
			class:textbox__readonly={type != 'select' && readonly}
			class:select-none={noselect}
			class:select-all={selectall}
			style:text-align={textAlign}
			bind:value
			bind:this={htmlInput}
			on:click
			on:mousedown
			on:input={(e) => {
				dispatch('input', e.currentTarget.value);

				isInputValid = e.currentTarget.checkValidity();
			}}
			on:change={(e) => dispatch('change', e.currentTarget.value)}
			on:keydown={(e) => dispatch('keydown', e)}
		/>

		{#if type == 'password'}
			<button
				class="textbox__show-hide-icon"
				on:click={() => {
					showPassword = !showPassword;
					htmlInput.focus();
				}}
			>
				<Icon name={showPassword ? 'eye-shown' : 'eye-hidden'} />
			</button>
		{/if}
	</div>
</div>

<style lang="postcss">
	.textbox {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: var(--size-6);
	}

	.textbox__input-wrap {
		position: relative;
	}

	.textbox__input {
		z-index: 1;
		position: relative;
		flex-grow: 1;
		height: var(--size-control-l);
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
		z-index: 2;
		pointer-events: none;
		position: absolute;
		top: 50%;
		color: var(--clr-theme-scale-ntrl-50);
		transform: translateY(-50%);
	}

	.textbox__show-hide-icon {
		z-index: 2;
		position: absolute;
		top: 50%;
		right: var(--size-6);
		color: var(--clr-theme-scale-ntrl-50);
		transform: translateY(-50%);
		display: flex;
		padding: var(--size-2) var(--size-4);
		border-radius: var(--radius-s);
		transition: background-color var(--transition-fast);

		&:hover,
		&:focus {
			color: var(--clr-theme-scale-ntrl-40);
			outline: none;
			background-color: color-mix(in srgb, transparent, var(--darken-tint-light));
		}
	}

	/* select */
	.textbox__input[type='select'] {
		cursor: pointer;
	}

	/* number */
	.textbox__input[type='number'] {
		appearance: textfield;
		-moz-appearance: textfield;
		padding-right: 4.25rem;
	}

	.textbox__input[type='number']::-webkit-inner-spin-button,
	.textbox__input[type='number']::-webkit-outer-spin-button {
		-webkit-appearance: none;
		margin: 0;
	}

	.textbox__count-actions {
		z-index: 2;
		position: absolute;
		top: 50%;
		right: var(--size-6);
		transform: translateY(-50%);
		display: flex;
	}

	.textbox__count-actions::before {
		content: '';
		position: absolute;
		top: 50%;
		left: -0.375rem;
		transform: translateY(-50%);
		width: 1px;
		height: 100%;
		background-color: var(--clr-theme-container-outline-light);
	}

	.textbox__count-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--size-2) var(--size-4);
		border-radius: var(--radius-s);
		color: var(--clr-theme-scale-ntrl-50);
		transition: background-color var(--transition-fast);

		&:hover,
		&:focus {
			outline: none;
			background-color: color-mix(in srgb, transparent, var(--darken-tint-light));
			color: var(--clr-theme-scale-ntrl-40);
		}
	}

	/* Modifiers */

	.textbox__left-orient {
		& .textbox__input {
			padding-left: calc(var(--size-32) + var(--size-2));
		}
		& .textbox__icon {
			left: var(--size-10);
		}
	}

	.textbox__right-orient {
		& .textbox__input {
			padding-right: calc(var(--size-32) + var(--size-2));
		}
		& .textbox__icon {
			right: var(--size-10);
		}
	}

	.textbox__readonly {
		background-color: var(--clr-theme-container-pale);
		border-color: var(--clr-theme-container-outline-light);
	}

	.wide {
		width: 100%;
		flex: 1;
	}
</style>
