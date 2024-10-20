<script lang="ts">
	import Toggle from '$lib/shared/Toggle.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		id?: string;
		label: string;
		checked: boolean;
		icon?: keyof typeof iconsJson;
		tooltip?: string;
		disabled?: boolean;
		onclick?: (e: MouseEvent) => void;
	}

	let { id, label, checked = $bindable(), icon, tooltip, disabled, onclick }: Props = $props();

	const toggleId = id || label.toLowerCase().replace(/\s/g, '-');
</script>

<Tooltip text={tooltip}>
	<label class="toggle-btn" class:disabled for={toggleId}>
		{#if icon}
			<div class="toggle-icon">
				<Icon name={icon} />
			</div>
		{/if}
		<span class="text-12 text-semibold toggle-btn__label">{label}</span>
		<Toggle
			id={toggleId}
			small
			{checked}
			on:click={(e) => {
				onclick?.(e);
			}}
		/>
	</label>
</Tooltip>

<style lang="postcss">
	.toggle-btn {
		width: fit-content;
		display: flex;
		align-items: center;
		gap: 10px;
		height: var(--size-button);
		padding: 4px 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		transition: border-color 0.2s;

		&:hover {
			border-color: var(--clr-border-1);
		}
	}

	.toggle-icon {
		display: flex;
		opacity: 0.5;
		margin-left: -2px;
		margin-right: -2px;
	}

	.toggle-btn__label {
		color: var(--clr-text-2);
	}
</style>
