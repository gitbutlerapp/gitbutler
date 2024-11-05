<script lang="ts">
	import { clickOutside } from '$lib/utils/clickOutside';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { onMount } from 'svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		element?: HTMLElement;
		id?: string;
		type?: inputType;
		icon?: keyof typeof iconsJson;
		value?: string;
		width?: number;
		textAlign?: 'left' | 'center' | 'right';
		placeholder?: string;
		helperText?: string;
		label?: string;
		reversedDirection?: boolean;
		wide?: boolean;
		minVal?: number;
		maxVal?: number;
		showCountActions?: boolean;
		disabled?: boolean;
		readonly?: boolean;
		required?: boolean;
		noselect?: boolean;
		selectall?: boolean;
		spellcheck?: boolean;
		autocorrect?: boolean;
		autocomplete?: boolean;
		autofocus?: boolean;
		onclick?: (e: MouseEvent & { currentTarget: EventTarget & HTMLInputElement }) => void;
		onmousedown?: (e: MouseEvent & { currentTarget: EventTarget & HTMLInputElement }) => void;
		oninput?: (val: string) => void;
		onchange?: (val: string) => void;
		onkeydown?: (e: KeyboardEvent & { currentTarget: EventTarget & HTMLInputElement }) => void;
	}

	let {
		element = $bindable(),
		id,
		type = 'text',
		icon,
		value = $bindable(),
		width,
		textAlign = 'left',
		placeholder,
		helperText,
		label,
		reversedDirection,
		wide,
		minVal,
		maxVal,
		showCountActions,
		disabled,
		readonly,
		required,
		noselect,
		selectall,
		spellcheck,
		autocorrect,
		autocomplete,
		autofocus,
		onclick,
		onmousedown,
		oninput,
		onchange,
		onkeydown
	}: Props = $props();

	let showPassword = $state(false);
	let isInputValid = $state(true);
	let htmlInput: HTMLInputElement;

	export function onClickOutside() {
		htmlInput.blur();
	}

	type inputType =
		| 'text'
		| 'password'
		| 'number'
		| 'select'
		| 'email'
		| 'tel'
		| 'url'
		| 'search'
		| 'date'
		| 'time';

	onMount(() => {
		if (selectall) htmlInput.select();
		else if (autofocus) htmlInput.focus();
	});
</script>

<div
	class="textbox"
	bind:this={element}
	class:wide
	style:width={width ? pxToRem(width) : undefined}
	class:wiggle-animation={!isInputValid}
	use:clickOutside={{ excludeElement: element, handler: onClickOutside }}
>
	{#if label}
		<label class="textbox__label text-13 text-semibold" for={id}>
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
			class="text-input textbox__input text-13"
			class:textbox__readonly={type !== 'select' && readonly}
			class:select-none={noselect}
			style:text-align={textAlign}
			bind:value
			bind:this={htmlInput}
			{onclick}
			{onmousedown}
			oninput={(e) => {
				oninput?.(e.currentTarget.value);
				isInputValid = e.currentTarget.checkValidity();
			}}
			onchange={(e) => {
				onchange?.(e.currentTarget.value);
			}}
			{onkeydown}
		/>

		{#if type === 'number' && showCountActions}
			<div class="textbox__count-actions">
				<button
					type="button"
					class="textbox__count-btn"
					onclick={() => {
						htmlInput.stepDown();

						oninput?.(htmlInput.value);
						onchange?.(htmlInput.value);

						isInputValid = htmlInput.checkValidity();
					}}
				>
					<Icon name="minus-small" />
				</button>
				<button
					type="button"
					class="textbox__count-btn"
					onclick={() => {
						htmlInput.stepUp();

						oninput?.(htmlInput.value);
						onchange?.(htmlInput.value);

						isInputValid = htmlInput.checkValidity();
					}}
				>
					<Icon name="plus-small" />
				</button>
			</div>
		{/if}

		{#if type === 'password'}
			<button
				type="button"
				class="textbox__show-hide-icon"
				onclick={() => {
					showPassword = !showPassword;
					htmlInput.focus();
				}}
			>
				<Icon name={showPassword ? 'eye-shown' : 'eye-hidden'} />
			</button>
		{/if}

		{#if helperText}
			<p class="text-11 text-body textbox__helper-text">{helperText}</p>
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

	.textbox__helper-text {
		color: var(--clr-scale-ntrl-50);
		margin-top: 6px;
	}

	.textbox__icon {
		display: flex;
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
		user-select: none;
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
