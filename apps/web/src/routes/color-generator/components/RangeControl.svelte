<script lang="ts">
	interface Props {
		label: string;
		min: number;
		max: number;
		value: number;
		oninput: () => void;
		suffix?: string;
	}

	let { label, min, max, value = $bindable(), oninput, suffix = '%' }: Props = $props();
</script>

<div class="scale-control">
	<div class="flex full-width">
		<span class="text-12 label">{label}:</span>
		<span class="text-12 range-value">{Math.round(value)}{suffix}</span>
	</div>
	<input type="range" {min} {max} bind:value {oninput} />
</div>

<style>
	.scale-control {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		width: 100%;
		gap: 12px;

		--range-thumb-size: 14px;
		--range-track-height: 2px;
	}

	.scale-control .label {
		flex: 1;
	}

	.scale-control .range-value,
	.scale-control .label {
		color: var(--card-ui-color);
		opacity: 0.8;
		pointer-events: none;
	}

	.scale-control input[type='range'] {
		-webkit-appearance: none;
		appearance: none;
		width: 100%;
		height: var(--range-track-height);
		border-radius: 2px;
		outline: none;
		background: var(--card-ui-color);
	}

	.scale-control input[type='range']::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--card-ui-color);
		cursor: pointer;
	}

	.scale-control input[type='range']::-moz-range-thumb {
		width: var(--range-thumb-size);
		height: var(--range-thumb-size);
		border-radius: 50%;
		background: var(--card-ui-color);
		cursor: pointer;
	}

	.scale-control input[type='range']::-moz-range-track {
		height: var(--range-track-height);
		border-radius: 2px;
		background: var(--card-ui-color);
	}
</style>
