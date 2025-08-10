<script lang="ts" module>
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	export interface Props {
		checked?: boolean;
		icon?: keyof typeof iconsJson;
		tooltip?: string; // Optional tooltip text
		value?: string;
		id?: string;
		testId?: string;
		onclick?: (e: MouseEvent) => void;
		onchange?: (checked: boolean) => void;
	}
</script>

<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import Tooltip from '$components/Tooltip.svelte';

	let {
		checked = $bindable(false),
		value,
		icon = 'draggable',
		tooltip,
		id,
		testId,
		onclick,
		onchange
	}: Props = $props();

	function handleClick(e: MouseEvent) {
		e.stopPropagation();
		checked = !checked;
		onclick?.(e);
		onchange?.(checked);
	}
</script>

<Tooltip text={tooltip}>
	<button
		type="button"
		role="switch"
		aria-checked={checked}
		data-testid={testId}
		class="switch"
		class:checked
		{id}
		onclick={handleClick}
	>
		<div class="switch-track">
			<div class="switch-thumb">
				<Icon name={icon} />
			</div>

			<div class="switch-indicator-on"></div>
			<div class="switch-indicator-off"></div>
		</div>

		{#if id && value}
			<input type="hidden" name={id} {value} {checked} />
		{/if}
	</button>
</Tooltip>

<style lang="postcss">
	.switch {
		display: inline-flex;
		appearance: none;
		position: relative;
		align-items: center;
		padding: 0;
		border: none;
		outline: none;
		background: none;
		cursor: pointer;

		&:focus-visible {
			outline: 2px solid var(--clr-theme-pop-element);
			outline-offset: 2px;
		}
	}

	.switch-track {
		display: inline-flex;
		position: relative;
		position: relative;
		width: 46px;
		height: var(--size-tag);
		overflow: hidden;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		box-shadow: inset 0 0 0 1px var(--clr-border-2);
		transition:
			background-color var(--transition-fast),
			box-shadow var(--transition-fast);
	}

	.switch.checked .switch-track {
		background-color: var(--clr-theme-pop-soft);
		box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--clr-theme-pop-element), transparent 50%);
	}

	.switch-thumb {
		display: flex;
		z-index: 1;
		position: absolute;
		top: 1px;
		left: 1px;
		align-items: center;
		justify-content: center;
		width: 26px;
		height: calc(100% - 2px);
		border-radius: calc(var(--radius-m) - 1px);
		background-color: var(--clr-bg-1);
		box-shadow: 0 0 0 1px var(--clr-border-2);
		color: var(--clr-text-2);
		transition: transform var(--transition-medium);
	}

	.switch.checked .switch-thumb {
		transform: translateX(18px);
	}

	.switch.checked .switch-indicator-on {
		box-shadow: inset 0 0 0 var(--line-width) var(--clr-theme-pop-element);
	}

	.switch-indicator-on,
	.switch-indicator-off {
		--line-width: 0.094rem;
		z-index: 0;
		position: absolute;
		top: 50%;
		transform: translateY(-50%);
		box-shadow: inset 0 0 0 var(--line-width) var(--clr-text-3);
		pointer-events: none;
		transition: box-shadow var(--transition-fast);
	}

	.switch-indicator-on {
		left: 7px;
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}

	.switch-indicator-off {
		right: 10px;
		width: var(--line-width);
		height: 10px;
		opacity: 0.5;
	}

	/* Hover states */
	.switch:hover .switch-track {
		background-color: var(--clr-bg-2-muted);
	}

	.switch.checked:hover .switch-track {
		background-color: var(--clr-theme-pop-soft-hover);
	}
</style>
