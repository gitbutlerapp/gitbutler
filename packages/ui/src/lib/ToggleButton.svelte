<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import Toggle from '$lib/Toggle.svelte';
	import Tooltip from '$lib/Tooltip.svelte';
	import iconsJson from '$lib/data/icons.json';

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
			onclick={(e) => {
				onclick?.(e);
			}}
		/>
	</label>
</Tooltip>

<style lang="postcss">
	.toggle-btn {
		--label-clr: var(--clr-btn-ntrl-outline-text);
		--btn-bg: var(--clr-btn-ntrl-outline-bg);
		--opacity-btn-bg: 0;
		--btn-border-clr: var(--clr-btn-ntrl-outline);
		--icon-opacity: var(--opacity-btn-icon-outline);
		--btn-border-opacity: var(--opacity-btn-outline);

		width: fit-content;
		display: flex;
		align-items: center;
		gap: 10px;
		height: var(--size-button);
		padding: 4px 8px;
		border-radius: var(--radius-m);
		transition: border-color 0.2s;

		color: var(--label-clr);
		background: color-mix(
			in srgb,
			var(--btn-bg, transparent),
			transparent calc((1 - var(--opacity-btn-bg, 1)) * 100%)
		);
		border: 1px solid
			color-mix(
				in srgb,
				var(--btn-border-clr, transparent),
				transparent calc((1 - var(--btn-border-opacity, 1)) * 100%)
			);

		&:hover {
			--icon-opacity: var(--opacity-btn-icon-outline-hover);
			--opacity-btn-bg: var(--opacity-btn-outline-bg-hover);
			--btn-border-opacity: var(--opacity-btn-outline);
		}
	}

	.toggle-icon {
		display: flex;
		margin-left: -2px;
		margin-right: -2px;
		opacity: var(--icon-opacity);
	}

	.toggle-btn__label {
		color: var(--clr-text-1);
	}
</style>
