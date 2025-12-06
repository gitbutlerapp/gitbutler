<script module lang="ts">
	import Toggle from '$components/Toggle.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Toggle',
		component: Toggle,
		args: {
			checked: false,
			disabled: false,
			small: false
		}
	});
</script>

<script lang="ts">
	let toggleValue = $state(false);
</script>

<Story name="Playground" />

<Story name="States">
	<div style="display: flex; flex-direction: column; gap: 16px;">
		<div style="display: flex; align-items: center; gap: 8px;">
			<Toggle checked={false} />
			<span>Unchecked</span>
		</div>
		<div style="display: flex; align-items: center; gap: 8px;">
			<Toggle checked={true} />
			<span>Checked</span>
		</div>
		<div style="display: flex; align-items: center; gap: 8px;">
			<Toggle checked={false} disabled={true} />
			<span>Disabled (unchecked)</span>
		</div>
		<div style="display: flex; align-items: center; gap: 8px;">
			<Toggle checked={true} disabled={true} />
			<span>Disabled (checked)</span>
		</div>
	</div>
</Story>

<Story name="Sizes">
	<div style="display: flex; flex-direction: column; gap: 16px;">
		<div style="display: flex; align-items: center; gap: 8px;">
			<Toggle checked={true} />
			<span>Default size</span>
		</div>
		<div style="display: flex; align-items: center; gap: 8px;">
			<Toggle checked={true} small={true} />
			<span>Small size</span>
		</div>
	</div>
</Story>

<Story name="Interactive">
	<div style="display: flex; flex-direction: column; gap: 16px;">
		<div style="display: flex; align-items: center; gap: 8px;">
			<Toggle
				bind:checked={toggleValue}
				onchange={(checked) => {
					console.warn('Toggle changed:', checked);
				}}
			/>
			<span>Current value: {toggleValue ? 'On' : 'Off'}</span>
		</div>
		<button
			type="button"
			onclick={() => {
				toggleValue = !toggleValue;
			}}
		>
			Toggle programmatically
		</button>
	</div>
</Story>
