<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import { clickOutside } from '$lib/utils/clickOutside';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { onMount, tick } from 'svelte';
	import type iconsJson from '$lib/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		element?: HTMLElement;
		id?: string;
		testId?: string;
		type?: inputType;
		iconLeft?: keyof typeof iconsJson;
		iconRight?: keyof typeof iconsJson;
		customIconLeft?: Snippet;
		customIconRight?: Snippet;
		size?: 'default' | 'large';
		textAlign?: 'left' | 'center' | 'right';
		value?: string;
		width?: number;
		placeholder?: string;
		helperText?: string;
		label?: string;
		wide?: boolean;
		minVal?: number;
		maxVal?: number;
		showCountActions?: boolean;
		disabled?: boolean;
		readonly?: boolean;
		required?: boolean;
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
		testId,
		type = 'text',
		iconLeft,
		iconRight,
		customIconLeft,
		customIconRight,
		value = $bindable(),
		width,
		size = 'default',
		textAlign = 'left',
		placeholder,
		helperText,
		label,
		wide,
		minVal,
		maxVal,
		showCountActions,
		disabled,
		readonly,
		required,
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

	export function focus() {
		htmlInput.focus();
	}

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
		else if (autofocus) {
			tick().then(() => {
				htmlInput.focus();
			});
		}
	});
</script>

<div
	class="textbox"
	bind:this={element}
	class:wide
	style:width={width ? `${pxToRem(width)}rem` : undefined}
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
		class:textbox__left-orient={iconLeft || customIconLeft}
		class:textbox__right-orient={iconRight || customIconRight}
		class:disabled
	>
		{#if customIconLeft}
			<div class="textbox__icon textbox__icon--left">
				{@render customIconLeft()}
			</div>
		{:else if iconLeft}
			<div class="textbox__icon textbox__icon--left">
				<Icon name={!disabled ? iconLeft : 'locked'} />
			</div>
		{/if}

		{#if customIconRight}
			<div class="textbox__icon textbox__icon--right">
				{@render customIconRight()}
			</div>
		{:else if iconRight}
			<div class="textbox__icon textbox__icon--right">
				<Icon name={!disabled ? iconRight : 'locked'} />
			</div>
		{/if}

		<input
			{id}
			data-testid={testId}
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
			class="text-input textbox__input size-{size} {size === 'large'
				? 'text-14 text-semibold'
				: 'text-13'}"
			class:readonly={type !== 'select' && readonly}
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
					disabled={disabled || readonly}
					onclick={() => {
						if (disabled || readonly) return;
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
					disabled={disabled || readonly}
					onclick={() => {
						if (disabled || readonly) return;
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
				disabled={disabled || readonly}
				onclick={() => {
					if (disabled || readonly) return;
					showPassword = !showPassword;
					htmlInput.focus();
				}}
			>
				<Icon name={showPassword ? 'eye-shown' : 'eye-hidden'} />
			</button>
		{/if}
	</div>

	{#if helperText}
		<p class="text-11 text-body textbox__helper-text">{helperText}</p>
	{/if}
</div>

<style lang="postcss">
	.textbox {
		display: flex;
		position: relative;
		flex-direction: column;
		gap: 6px;

		&.wide {
			flex: 1;
			width: 100%;
		}
	}

	.textbox__input-wrap {
		position: relative;

		&.disabled {
			& .textbox__icon {
				color: var(--clr-scale-ntrl-60);
			}

			& .textbox__input {
				border: 1px solid var(--clr-border-3);
				background-color: var(--clr-bg-1-muted);
				color: var(--clr-text-2);
			}
		}
	}

	.textbox__input {
		position: relative;
		flex-grow: 1;
		width: 100%;

		&.readonly {
			border-color: var(--clr-border-2);
			background-color: var(--clr-bg-1-muted);
		}

		&.size-default {
			height: var(--size-cta);
		}

		&.size-large {
			height: auto;
			padding: 8px 10px;
		}
	}

	.textbox__label {
		color: var(--clr-scale-ntrl-50);
	}

	.textbox__helper-text {
		color: var(--clr-scale-ntrl-50);
	}

	.textbox__icon {
		display: flex;
		z-index: var(--z-ground);
		position: absolute;
		top: 50%;
		transform: translateY(-50%);
		color: var(--clr-scale-ntrl-50);
		pointer-events: none;
	}

	.textbox__show-hide-icon {
		display: flex;
		z-index: var(--z-ground);
		position: absolute;
		top: 50%;
		right: 6px;
		padding: 2px 4px;
		transform: translateY(-50%);
		border-radius: var(--radius-s);
		color: var(--clr-scale-ntrl-50);
		transition: background-color var(--transition-fast);

		&:hover,
		&:focus {
			outline: none;
			background-color: var(--clr-bg-2);
			color: var(--clr-scale-ntrl-40);
		}

		&:disabled {
			color: var(--clr-scale-ntrl-60);
			cursor: not-allowed;
			opacity: 0.5;

			&:hover,
			&:focus {
				background-color: transparent;
				color: var(--clr-scale-ntrl-60);
			}
		}
	}

	/* select */
	.textbox__input[type='select']:not([disabled]),
	.textbox__input[type='select']:not([readonly]) {
		cursor: pointer;
		user-select: none;
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
		display: flex;
		z-index: var(--z-ground);
		position: absolute;
		top: 50%;
		right: 6px;
		transform: translateY(-50%);
	}

	.textbox__count-actions::before {
		position: absolute;
		top: 50%;
		left: -6px;
		width: 1px;
		height: 100%;
		transform: translateY(-50%);
		background-color: var(--clr-border-2);
		content: '';
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

		&:disabled {
			color: var(--clr-scale-ntrl-60);
			cursor: not-allowed;
			opacity: 0.5;

			&:hover,
			&:focus {
				background-color: transparent;
				color: var(--clr-scale-ntrl-60);
			}
		}
	}

	/* Icon positioning */
	.textbox__icon--left {
		left: 10px;
	}

	.textbox__icon--right {
		right: 10px;
	}

	/* MODIFIERS */
	.textbox__left-orient {
		& .textbox__input {
			padding-left: 34px;
		}
	}

	.textbox__right-orient {
		& .textbox__input {
			padding-right: 34px;
		}
	}

	/* Handle both icons present */
	.textbox__left-orient.textbox__right-orient {
		& .textbox__input {
			padding-right: 34px;
			padding-left: 34px;
		}
	}
</style>
