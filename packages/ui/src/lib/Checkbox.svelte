<script lang="ts" module>
	export type CheckboxStyle = 'default' | 'neutral';
	export interface CheckboxProps {
		name?: string;
		small?: boolean;
		disabled?: boolean;
		checked?: boolean;
		value?: string;
		indeterminate?: boolean;
		style?: CheckboxStyle;
		onclick?: (e: MouseEvent) => void;
		onchange?: (
			e: Event & {
				currentTarget: EventTarget & HTMLInputElement;
			}
		) => void;
	}
</script>

<script lang="ts">
	let input: HTMLInputElement;

	let {
		name,
		small = false,
		disabled = false,
		checked = $bindable(),
		value = '',
		indeterminate = false,
		style = 'default',
		onclick,
		onchange
	}: CheckboxProps = $props();

	$effect(() => {
		if (input) input.indeterminate = indeterminate;
	});
</script>

<input
	bind:this={input}
	bind:checked
	onclick={(e) => {
		e.stopPropagation();
		onclick?.(e);
	}}
	onchange={(e) => {
		e.stopPropagation();
		onchange?.(e);
	}}
	type="checkbox"
	class={`checkbox ${style}`}
	class:small
	{value}
	id={name}
	{name}
	{disabled}
/>

<style lang="postcss">
	.checkbox {
		appearance: none;
		cursor: pointer;
		width: 16px;
		height: 16px;
		flex-shrink: 0;
		border-radius: var(--radius-s);
		background-color: var(--clr-bg-1);
		box-shadow: inset 0 0 0 1px var(--clr-border-2);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast),
			opacity var(--transition-fast),
			transform var(--transition-fast);
		position: relative;

		/* not checked */
		&:hover,
		&:focus {
			outline: none;

			&::after {
				opacity: 0.8;
				transform: scale(0.8);
			}
		}

		&:disabled {
			pointer-events: none;
			opacity: 0.3;
			cursor: not-allowed;
			background-color: var(--clr-scale-ntrl-60);
			border-color: none;
		}

		/* indeterminate */

		&:indeterminate::before {
			content: '';
			position: absolute;
			width: 50%;
			height: 2px;
			top: 50%;
			left: 50%;
			transform: translate(-50%, -50%);
		}

		&.default:indeterminate {
			background-color: var(--clr-theme-pop-element);
			box-shadow: inset 0 0 0 1px var(--clr-theme-pop-element);

			&:hover {
				background-color: var(--clr-theme-pop-element-hover);
			}

			&::before {
				background-color: white;
			}
		}

		&.neutral:indeterminate {
			background-color: var(--clr-bg-2);

			&:hover {
				background-color: var(--clr-bg-3);
			}

			&::before {
				background-color: var(--clr-scale-ntrl-30);
			}
		}

		/* checked */
		&:checked {
			&:disabled {
				pointer-events: none;
				opacity: 0.4;
				cursor: not-allowed;
			}

			&::after {
				opacity: 1;
				filter: brightness(2);
				transform: scale(1);
			}
		}

		&.default:checked {
			background-color: var(--clr-theme-pop-element);
			box-shadow: inset 0 0 0 1px var(--clr-theme-pop-element);

			&:hover {
				background-color: var(--clr-theme-pop-element-hover);
			}
		}

		&.neutral:checked {
			background-color: var(--clr-bg-2);
			box-shadow: inset 0 0 0 1px var(--clr-scale-ntrl-30);

			&:hover {
				background-color: var(--clr-bg-3);
			}
		}

		&::after {
			content: '';
			position: absolute;
			width: 100%;
			height: 100%;
			border-radius: var(--radius-s);
			transition:
				opacity var(--transition-fast),
				transform var(--transition-fast);
			opacity: 0;
		}

		/* tick element */
		&:not(:indeterminate)::after {
			background-image: url('data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTAiIGhlaWdodD0iNyIgdmlld0JveD0iMCAwIDEwIDciIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+CjxwYXRoIGQ9Ik05IDEuNUw0LjkyMTM5IDUuNzQ4NTVDNC41Mjc4MyA2LjE1ODUxIDMuODcyMTcgNi4xNTg1MSAzLjQ3ODYxIDUuNzQ4NTZMMSAzLjE2NjY3IiBzdHJva2U9IiNBNUE1QTUiIHN0cm9rZS13aWR0aD0iMS41Ii8+Cjwvc3ZnPgo=');
			background-position: center;
			background-size: 80%;
			background-repeat: no-repeat;
		}

		/* modifiers */

		&.small {
			width: 14px;
			height: 14px;
		}
	}
</style>
