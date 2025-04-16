<script lang="ts">
	interface Props {
		name?: string;
		small?: boolean;
		disabled?: boolean;
		value?: string;
		id?: string;
		checked?: boolean;
		onchange?: (e: Event) => void;
	}

	let {
		name = '',
		small = false,
		disabled = false,
		value = '',
		id = '',
		checked = $bindable(),
		onchange
	}: Props = $props();
</script>

<input
	type="radio"
	class="focus-state radio"
	class:small
	{id}
	{value}
	{name}
	{disabled}
	{checked}
	{onchange}
	onkeydown={(e) => {
		if (e.key === 'Enter') {
			e.preventDefault();
			e.stopPropagation();
			checked = true;
			onchange?.(e);
		}
	}}
/>

<style lang="postcss">
	.radio {
		appearance: none;
		cursor: pointer;
		width: 16px;
		height: 16px;
		border-radius: 16px;
		background-color: var(--clr-bg-1);
		box-shadow: inset 0 0 0 1px var(--clr-border-2);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast),
			opacity var(--transition-fast),
			transform var(--transition-fast);
		position: relative;

		/* disabled */
		&:not(:disabled)&:not(:checked):hover {
			box-shadow: inset 0 0 0 1px var(--clr-border-1);

			&::after {
				opacity: 0.2;
				transform: scale(0.7);
			}
		}

		&:disabled {
			opacity: 0.4;
			cursor: not-allowed;
			background-color: var(--clr-scale-ntrl-70);
		}

		/* checked */
		&:checked {
			background-color: var(--clr-theme-pop-element);
			box-shadow: inset 0 0 0 1px var(--clr-theme-pop-element);

			&::after {
				opacity: 1;
				transform: scale(1);
				background-color: var(--clr-core-ntrl-100);
			}
		}

		/* tick element */
		&::after {
			content: '';
			position: absolute;
			top: 4px;
			left: 4px;
			width: calc(100% - 8px);
			height: calc(100% - 8px);
			border-radius: 16px;
			background-color: var(--clr-scale-ntrl-0);
			transition: background-color var(--transition-fast);
			transform: scale(0.5);
			opacity: 0;
		}

		/* modifiers */

		&.small {
			width: 14px;
			height: 14px;
		}
	}
</style>
