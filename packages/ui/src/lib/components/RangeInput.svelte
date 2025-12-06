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

	// Calculate the percentage for the fill effect
	let percentage = $derived(((value - min) / (max - min)) * 100);
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

	<div class="wrapper">
		<div class="scale" style:--scale-percentage={percentage}>
			<div class="scale__track"></div>
			<div class="scale__start"></div>
			<div class="scale__thumb"></div>
		</div>

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
			oninput={(e) => {
				oninput?.(parseFloat(e.currentTarget.value));
			}}
			onchange={(e) => {
				onchange?.(parseFloat(e.currentTarget.value));
			}}
		/>
	</div>

	{#if error}
		<p class="text-11 text-body error-text">{error}</p>
	{:else if helperText}
		<p class="text-11 text-body helper-text">{helperText}</p>
	{/if}
</div>

<style lang="postcss">
	.range-input {
		display: flex;
		position: relative;
		flex-direction: column;
		gap: 8px;

		--range-track: 4px;
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

	.wrapper {
		display: flex;
		position: relative;
		align-items: center;
		width: 100%;
		height: var(--range-thumb-size);
	}

	.scale {
		position: absolute;
		top: 50%;
		left: 0;
		width: max(var(--range-thumb-size), calc(var(--scale-percentage) * 1%));
		height: var(--range-thumb-size);
		transform: translateY(-50%);
		pointer-events: none;
		transition: width var(--transition-fast);
	}

	.scale__track {
		position: absolute;
		top: 50%;
		left: 0;
		width: 100%;
		height: 100%;
		transform: translateY(-50%);
		background: var(--range-accent-color);
		clip-path: polygon(
			calc(var(--range-track) / 2) calc(50% - var(--range-track) / 2),
			calc(100% - var(--range-thumb-size) / 2) calc(50% - var(--range-thumb-size) / 2),
			calc(100% - var(--range-thumb-size) / 2) calc(50% + var(--range-thumb-size) / 2),
			calc(var(--range-track) / 2) calc(50% + var(--range-track) / 2)
		);
	}

	.scale__start {
		position: absolute;
		top: 50%;
		left: 0;
		width: var(--range-track);
		height: var(--range-track);
		transform: translateY(-50%);
		border-radius: 50%;
		background: var(--range-accent-color);
	}

	.scale__thumb {
		position: absolute;
		top: 50%;
		right: 0;
		width: var(--range-thumb-size);
		height: var(--range-thumb-size);
		transform: translateY(-50%);
		border: 2px solid var(--range-accent-color);
		border-radius: 50%;
		background: var(--clr-scale-ntrl-100);
	}

	.input {
		appearance: none;
		width: 100%;
		height: var(--range-track);
		border-radius: var(--range-track);
		outline: none;
		background: var(--clr-border-2);
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
			opacity: 0;
		}

		/* Firefox thumb styles */
		&::-moz-range-thumb {
			appearance: none;
			width: var(--range-thumb-size);
			height: var(--range-thumb-size);
			opacity: 0;
		}
	}

	/* Disabled state */
	.range-input:has(.input:disabled) {
		& .scale {
			opacity: 0.4;
		}

		& .scale__track,
		& .scale__start {
			background-color: var(--clr-scale-ntrl-60);
		}

		& .scale__thumb {
			border-color: var(--clr-scale-ntrl-60);
		}
	}

	/* Hover state */
	.range-input:has(.input:hover:not(:disabled)) {
		--range-accent-color: var(--clr-pop-hover);
	}

	/* Error state */
	.range-input.error {
		--range-accent-color: var(--clr-theme-err-element);

		& .scale__track,
		& .scale__start {
			background-color: var(--clr-theme-err-element);
		}

		& .scale__thumb {
			border-color: var(--clr-theme-err-element);
		}
	}
</style>
