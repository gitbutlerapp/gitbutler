<script lang="ts">
	import { focusable } from '$lib/focus/focusable';
	import { pxToRem } from '$lib/utils/pxToRem';

	interface Props {
		element?: HTMLInputElement;
		id?: string;
		testId?: string;
		value?: number;
		min?: number;
		max?: number;
		step?: number;
		label?: string;
		helperText?: string;
		error?: string;
		disabled?: boolean;
		showValue?: boolean;
		wide?: boolean;
		width?: number;
		oninput?: (val: number) => void;
		onchange?: (val: number) => void;
	}

	let {
		element = $bindable(),
		id,
		testId,
		value = $bindable(0),
		min = 0,
		max = 100,
		step = 1,
		label,
		helperText,
		error,
		disabled,
		showValue = false,
		wide,
		width,
		oninput,
		onchange
	}: Props = $props();

	// Calculate the fill ratio (0-1)
	let fillRatio = $derived((value - min) / (max - min));
</script>

<div
	class="range-input"
	class:wide
	class:error={!!error}
	style:width={width ? `${pxToRem(width)}rem` : undefined}
>
	{#if label}
		<div class="label-wrap">
			<label class="label text-13 text-semibold" for={id}>
				{label}
			</label>
			{#if showValue}
				<span class="value text-13 text-semibold">{value}</span>
			{/if}
		</div>
	{/if}

	<input
		bind:this={element}
		{id}
		data-testid={testId}
		type="range"
		class="input"
		{min}
		{max}
		{step}
		{disabled}
		bind:value
		use:focusable={{ button: true }}
		style:--fill-ratio={fillRatio}
		oninput={(e) => {
			oninput?.(parseFloat(e.currentTarget.value));
		}}
		onchange={(e) => {
			onchange?.(parseFloat(e.currentTarget.value));
		}}
	/>

	{#if error}
		<p class="text-11 text-body error-text">{error}</p>
	{:else if helperText}
		<p class="text-11 text-body helper-text">{helperText}</p>
	{/if}
</div>

<style lang="postcss">
	.range-input {
		display: flex;
		flex-direction: column;
		gap: 8px;

		--range-track-height: 4px;
		--range-thumb-size: 16px;
		--range-accent-color: var(--clr-theme-pop-element);

		&.wide {
			flex: 1;
			width: 100%;
		}
	}

	.label-wrap {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.label {
		color: var(--clr-scale-ntrl-50);
	}

	.value {
		color: var(--clr-text-2);
	}

	.helper-text {
		color: var(--clr-scale-ntrl-50);
	}

	.error-text {
		color: var(--clr-theme-err-element);
	}

	.input {
		appearance: none;
		width: 100%;
		height: var(--range-track-height);
		border-radius: calc(var(--range-track-height) / 2);
		outline: none;
		background: linear-gradient(
			to right,
			var(--range-accent-color) 0%,
			var(--range-accent-color) calc(var(--fill-ratio) * 100%),
			var(--clr-border-2) calc(var(--fill-ratio) * 100%),
			var(--clr-border-2) 100%
		);
		cursor: pointer;

		&:focus-visible {
			outline: 2px solid var(--clr-theme-pop-element);
			outline-offset: 2px;
		}

		/* Webkit thumb styles */
		&::-webkit-slider-thumb {
			appearance: none;
			width: var(--range-thumb-size);
			height: var(--range-thumb-size);
			border-radius: 50%;
			background: var(--range-accent-color);
			cursor: pointer;
		}

		/* Firefox thumb styles */
		&::-moz-range-thumb {
			appearance: none;
			width: var(--range-thumb-size);
			height: var(--range-thumb-size);
			border: 2px solid var(--range-accent-color);
			border-radius: 50%;
			background: var(--clr-scale-ntrl-100);
			cursor: pointer;
		}

		&:disabled {
			cursor: not-allowed;
			opacity: 0.4;

			&::-webkit-slider-thumb {
				cursor: not-allowed;
			}

			&::-moz-range-thumb {
				cursor: not-allowed;
			}
		}
	}

	/* Hover state */
	.range-input:has(.input:hover:not(:disabled)) {
		--range-accent-color: var(--clr-pop-hover);
	}

	/* Error state */
	.range-input.error {
		--range-accent-color: var(--clr-theme-err-element);
	}
</style>
