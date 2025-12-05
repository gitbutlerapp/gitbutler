<script lang="ts">
	import RangeControl from './RangeControl.svelte';
	import { hslToHex } from '../utils/colorConversion';
	import { Icon } from '@gitbutler/ui';
	import type { ColorScale } from '../types/color';

	interface Props {
		scale: ColorScale;
		saturation: number;
		shade50Lightness: number;
		hue: number | null;
		onHueChange: (scaleId: string, hexColor: string) => void;
		onSaturationChange: (scaleId: string, value: number) => void;
		onShade50LightnessChange: (scaleId: string, value: number) => void;
		onCopyJSON: (scaleId: string) => void;
		onMinimize?: () => void;
	}

	let {
		scale,
		saturation = $bindable(),
		shade50Lightness = $bindable(),
		hue,
		onHueChange,
		onSaturationChange,
		onShade50LightnessChange,
		onCopyJSON,
		onMinimize
	}: Props = $props();

	const displayHue = $derived(hue !== null ? hue : scale.baseHue || 180);
	const colorPickerValue = $derived(hslToHex(displayHue, 0.7, 0.5));

	let colorPickerInput: HTMLInputElement;

	function openColorPicker() {
		colorPickerInput?.click();
	}

	function handleHueChange(e: Event) {
		const target = e.currentTarget as HTMLInputElement;
		onHueChange(scale.id, target.value);
	}

	function handleSaturationChange() {
		onSaturationChange(scale.id, saturation);
	}

	function handleShade50LightnessChange() {
		onShade50LightnessChange(scale.id, shade50Lightness);
	}
</script>

<div class="scale-header">
	<input
		type="color"
		bind:this={colorPickerInput}
		value={colorPickerValue}
		oninput={handleHueChange}
		class="hidden-color-picker"
	/>
	<div class="scale-actions">
		<button type="button" class="scale-control" title="Minimize" onclick={onMinimize}>
			<Icon name="minus-small" />
		</button>
		<button type="button" class="scale-control" title="Edit Hue" onclick={openColorPicker}>
			<Icon name="edit" />
		</button>
		<button
			type="button"
			class="scale-control"
			onclick={() => onCopyJSON(scale.id)}
			title="Copy Scale JSON"
		>
			<Icon name="copy-small" />
		</button>
	</div>

	<div class="stack-v gap-8">
		<span class="text-15 text-body text-bold scale-name">{scale.name}</span>

		<RangeControl
			label="Saturation"
			min={0}
			max={100}
			bind:value={saturation}
			oninput={handleSaturationChange}
		/>
		<RangeControl
			label="Mid Lightness"
			min={30}
			max={60}
			bind:value={shade50Lightness}
			oninput={handleShade50LightnessChange}
		/>
	</div>
</div>

<style>
	.scale-header {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		height: 100%;
		padding: 16px 16px 24px;
		gap: 12px;
	}

	.scale-name {
		color: var(--card-ui-color);
	}

	.scale-actions {
		display: flex;
		justify-content: flex-end;
		width: 100%;
		gap: 4px;
	}

	.scale-control {
		display: flex;
		align-items: center;
		padding: 4px;
		border-radius: var(--radius-btn);
		color: var(--card-ui-color);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: color-mix(
				in srgb,
				#000 calc((var(--opacity-btn-solid-hover) * 100%)),
				transparent
			);
		}
	}

	.hidden-color-picker {
		position: absolute;
		width: 0;
		height: 0;
		opacity: 0;
		pointer-events: none;
	}
</style>
