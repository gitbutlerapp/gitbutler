<script lang="ts">
	import RangeControl from "./RangeControl.svelte";
	import type { ColorScale } from "../types/color";

	interface Props {
		scale: ColorScale;
		saturation: number;
		shade50Lightness: number;
		hue: number | null;
		onHueChange: (scaleId: string, hue: number) => void;
		onSaturationChange: (scaleId: string, value: number) => void;
		onShade50LightnessChange: (scaleId: string, value: number) => void;
	}

	let {
		scale,
		saturation = $bindable(),
		shade50Lightness = $bindable(),
		hue = $bindable(),
		onHueChange,
		onSaturationChange,
		onShade50LightnessChange,
	}: Props = $props();

	let displayHue = $state(hue !== null ? hue : scale.baseHue || 180);

	$effect(() => {
		if (hue !== null) {
			displayHue = hue;
		}
	});

	function handleHueChangeFromRange() {
		displayHue = Math.round(displayHue);
		hue = displayHue;
		onHueChange(scale.id, displayHue);
	}

	function handleSaturationChange() {
		onSaturationChange(scale.id, saturation);
	}

	function handleShade50LightnessChange() {
		onShade50LightnessChange(scale.id, shade50Lightness);
	}
</script>

<div class="scale-header">
	<div class="stack-v gap-14">
		<span class="text-15 text-body text-bold scale-name">{scale.name}</span>

		<RangeControl
			label="Hue"
			min={0}
			max={360}
			bind:value={displayHue}
			oninput={handleHueChangeFromRange}
			suffix="°"
		/>
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
		padding: 24px 16px;
		gap: 12px;
	}

	.scale-name {
		color: var(--card-ui-color);
	}
</style>
