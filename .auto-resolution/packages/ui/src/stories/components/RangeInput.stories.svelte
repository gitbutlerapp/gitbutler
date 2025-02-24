<script module lang="ts">
	import RangeInput from '$components/RangeInput.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Range Input',
		component: RangeInput,
		args: {
			value: 50,
			min: 0,
			max: 100,
			step: 1,
			label: '',
			helperText: '',
			error: '',
			disabled: false,
			showValue: false,
			wide: false,
			width: undefined
		},
		argTypes: {
			min: {
				control: { type: 'number' }
			},
			max: {
				control: { type: 'number' }
			},
			step: {
				control: { type: 'number' }
			},
			width: {
				control: { type: 'number' }
			}
		}
	});
</script>

<script lang="ts">
	let basicValue = $state(50);
	let volumeValue = $state(75);
	let opacityValue = $state(0.8);
	let temperatureValue = $state(20);
	let errorValue = $state(150);

	// Dynamic error computation
	let errorMessage = $derived(errorValue > 100 ? 'Value cannot exceed 100' : '');
</script>

<Story name="Default">
	{#snippet template(args)}
		<div class="wrap">
			<RangeInput
				bind:value={basicValue}
				min={args.min}
				max={args.max}
				step={args.step}
				label={args.label}
				helperText={args.helperText}
				error={args.error}
				disabled={args.disabled}
				showValue={args.showValue}
				wide={args.wide}
				width={args.width}
			/>
		</div>
	{/snippet}
</Story>

<Story name="With Label and Value Display">
	{#snippet template()}
		<div class="wrap">
			<RangeInput
				bind:value={volumeValue}
				min={0}
				max={100}
				step={5}
				label="Volume"
				showValue={true}
				helperText="Adjust the volume level"
			/>
		</div>
	{/snippet}
</Story>

<Story name="With Custom Range">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Temperature (°C)</h4>
				<RangeInput
					bind:value={temperatureValue}
					min={-10}
					max={40}
					step={0.5}
					label="Temperature"
					showValue={true}
				/>
			</div>
			<div class="story-group">
				<h4>Opacity</h4>
				<RangeInput
					bind:value={opacityValue}
					min={0}
					max={1}
					step={0.05}
					label="Opacity"
					showValue={true}
				/>
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Disabled">
	{#snippet template()}
		<div class="wrap">
			<RangeInput
				value={60}
				min={0}
				max={100}
				label="Disabled Range"
				showValue={true}
				disabled={true}
				helperText="This range input is disabled"
			/>
		</div>
	{/snippet}
</Story>

<Story name="With Error">
	{#snippet template()}
		<div class="wrap">
			<RangeInput
				bind:value={errorValue}
				min={0}
				max={200}
				step={10}
				label="Value with validation"
				showValue={true}
				error={errorMessage}
			/>
			<p class="text-13" style="margin-top: 12px; color: var(--clr-scale-ntrl-40);">
				Move the slider above 100 to see the error state
			</p>
		</div>
	{/snippet}
</Story>

<Story name="Wide">
	{#snippet template()}
		<div class="wrap">
			<RangeInput
				bind:value={basicValue}
				min={0}
				max={100}
				label="Wide Range Input"
				showValue={true}
				wide={true}
				helperText="This input takes the full width of its container"
			/>
		</div>
	{/snippet}
</Story>

<Story name="Various Sizes">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Default Width</h4>
				<RangeInput value={50} min={0} max={100} label="Default" showValue={true} />
			</div>
			<div class="story-group">
				<h4>Custom Width (200px)</h4>
				<RangeInput value={50} min={0} max={100} label="Custom" showValue={true} width={200} />
			</div>
			<div class="story-group">
				<h4>Custom Width (400px)</h4>
				<RangeInput value={50} min={0} max={100} label="Wider" showValue={true} width={400} />
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Interactive Examples">
	{#snippet template()}
		<div class="wrap">
			<div class="story-group">
				<h4>Volume Control</h4>
				<RangeInput
					bind:value={volumeValue}
					min={0}
					max={100}
					step={1}
					label="Volume"
					showValue={true}
					wide={true}
				/>
			</div>
			<div class="story-group">
				<h4>Brightness</h4>
				<RangeInput
					bind:value={basicValue}
					min={0}
					max={100}
					step={5}
					label="Brightness"
					showValue={true}
					wide={true}
				/>
			</div>
			<div class="story-group">
				<h4>Fine Control (Small Steps)</h4>
				<RangeInput
					bind:value={opacityValue}
					min={0}
					max={1}
					step={0.01}
					label="Opacity"
					showValue={true}
					wide={true}
					helperText="Fine-grained control with 0.01 step"
				/>
			</div>
		</div>
	{/snippet}
</Story>

<style>
	.wrap {
		display: flex;
		flex-direction: column;
		min-width: 320px;
		padding: 24px;
		gap: 24px;
	}

	.story-group {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	h4 {
		margin: 0;
		color: var(--clr-scale-ntrl-30);
		font-weight: 600;
		font-size: 14px;
	}
</style>
