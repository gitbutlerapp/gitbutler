<script lang="ts">
	import Icon from '$lib/shared/Icon.svelte';
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
	export let showCountActions = false;
	export let disabled = false;
	export let readonly = false;
	export let required = false;
	export let noselect = false;
	export let selectall = false;
	export let spellcheck = false;
	export let autocorrect = false;
	export let autocomplete = false;
	export let focus = false;

	export let type: 'text' | 'password' | 'select' | 'number' | 'email' = 'text';

	const dispatch = createEventDispatcher<{
		input: string;
		change: string;
		keydown: KeyboardEvent;
	}>();

	let showPassword = false;
	let isInputValid = true;
	let htmlInput: HTMLInputElement;

	onMount(() => {
		if (selectall) htmlInput.select();
		else if (focus) htmlInput.focus();
	});
</script>

<div
	class="textbox"
	bind:this={element}
	class:wide
	style:width={pxToRem(width)}
	class:wiggle={!isInputValid}
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
				<Icon name={!disabled ? icon : 'locked'} />
			</div>
		{/if}

		<input
			{id}
			{readonly}
			{required}
			{placeholder}
			{spellcheck}
			{disabled}
			autocorrect={autocorrect ? 'on' : 'off'}
			autocomplete={autocomplete ? 'on' : 'off'}
			min={minVal}
			max={maxVal}
			{...type === 'password' && showPassword ? { type: 'text' } : { type }}
			class:show-count-actions={showCountActions}
			class="text-input textbox__input text-base-13"
			class:textbox__readonly={type !== 'select' && readonly}
			class:select-none={noselect}
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

		{#if type === 'number' && showCountActions}
			<div class="textbox__count-actions">
				<button
					class="textbox__count-btn"
					on:click={() => {
						htmlInput.stepDown();
						dispatch('input', htmlInput.value);
						dispatch('change', htmlInput.value);

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
						dispatch('change', htmlInput.value);

						isInputValid = htmlInput.checkValidity();
					}}
				>
					<Icon name="plus-small" />
				</button>
			</div>
		{/if}

		{#if type === 'password'}
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
		gap: 6px;
	}

	.textbox__input-wrap {
		position: relative;

		&.disabled {
			/* background-color: var(--clr-bg-1); */
			& .textbox__icon {
				color: var(--clr-scale-ntrl-60);
			}

			& .textbox__input {
				color: var(--clr-text-2);
				background-color: var(--clr-bg-2);
				border: 1px solid var(--clr-border-3);
			}
		}
	}

	.textbox__input {
		position: relative;
		flex-grow: 1;
		height: var(--size-cta);
		width: 100%;
	}

	.textbox__label {
		color: var(--clr-scale-ntrl-50);
	}

	.textbox__icon {
		z-index: var(--z-ground);
		pointer-events: none;
		position: absolute;
		top: 50%;
		color: var(--clr-scale-ntrl-50);
		transform: translateY(-50%);
	}

	.textbox__show-hide-icon {
		z-index: var(--z-ground);
		position: absolute;
		top: 50%;
		right: 6px;
		color: var(--clr-scale-ntrl-50);
		transform: translateY(-50%);
		display: flex;
		padding: 2px 4px;
		border-radius: var(--radius-s);
		transition: background-color var(--transition-fast);

		&:hover,
		&:focus {
			color: var(--clr-scale-ntrl-40);
			outline: none;
			background-color: var(--clr-bg-2);
		}
	}

	/* select */
	.textbox__input[type='select']:not([disabled]),
	.textbox__input[type='select']:not([readonly]) {
		cursor: pointer;
	}

	/* number */
	.textbox__input[type='number'] {
		appearance: textfield;
		-moz-appearance: textfield;

		&.show-count-actions {
			padding-right: 68px;
		}
	}

	.textbox__input[type='number']::-webkit-inner-spin-button,
	.textbox__input[type='number']::-webkit-outer-spin-button {
		-webkit-appearance: none;
		margin: 0;
	}

	.textbox__count-actions {
		z-index: var(--z-ground);
		position: absolute;
		top: 50%;
		right: 6px;
		transform: translateY(-50%);
		display: flex;
	}

	.textbox__count-actions::before {
		content: '';
		position: absolute;
		top: 50%;
		left: -6px;
		transform: translateY(-50%);
		width: 1px;
		height: 100%;
		background-color: var(--clr-border-2);
	}

	.textbox__count-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2px 4px;
		border-radius: var(--radius-s);
		color: var(--clr-scale-ntrl-50);
		transition: background-color var(--transition-fast);

		&:hover,
		&:focus {
			outline: none;
			background-color: var(--clr-bg-1-muted);
			color: var(--clr-scale-ntrl-40);
		}
	}

	/* Modifiers */

	.textbox__left-orient {
		& .textbox__input {
			padding-left: 34px;
		}
		& .textbox__icon {
			left: 10px;
		}
	}

	.textbox__right-orient {
		& .textbox__input {
			padding-right: 34px;
		}
		& .textbox__icon {
			right: 10px;
		}
	}

	.textbox__readonly {
		background-color: var(--clr-bg-2);
		border-color: var(--clr-border-2);
	}

	.wide {
		width: 100%;
		flex: 1;
	}
</style>
