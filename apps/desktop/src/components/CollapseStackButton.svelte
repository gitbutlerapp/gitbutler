<script lang="ts">
	interface Props {
		onclick: () => void;
		disabled?: boolean;
		isFolded?: boolean;
	}

	const { onclick, disabled, isFolded }: Props = $props();
</script>

<button
	class="collapse-button"
	class:isFolded
	type="button"
	aria-label="Collapse stack"
	{onclick}
	{disabled}
>
	<svg class="collapse-icon" viewBox="0 0 15 10" fill="none" xmlns="http://www.w3.org/2000/svg">
		<path
			d="M11.75 0.75H2.75C1.64543 0.75 0.75 1.64543 0.75 2.75V6.75C0.75 7.85457 1.64543 8.75 2.75 8.75H11.75C12.8546 8.75 13.75 7.85457 13.75 6.75V2.75C13.75 1.64543 12.8546 0.75 11.75 0.75Z"
			stroke="currentColor"
			stroke-width="1.5"
			stroke-linecap="round"
		/>
		<rect
			class="collapse-icon__lane"
			x="0.75"
			y="0.75"
			width="5"
			height="8"
			rx="2"
			stroke="currentColor"
			stroke-width="1.5"
		/>
	</svg>
</button>

<style lang="postcss">
	.collapse-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}

		&:not(.isFolded):hover {
			& .collapse-icon__lane {
				fill: currentColor;
			}
		}

		&:disabled {
			cursor: not-allowed;
			opacity: 0.5;
		}

		&.isFolded {
			.collapse-icon__lane {
				fill: currentColor;
			}
		}

		&.isFolded:hover {
			cursor: pointer;
			.collapse-icon__lane {
				fill: none;
			}
		}
	}

	.collapse-icon {
		position: relative;
		width: 15px;
		height: 10px;
		--line-width: 0.094rem;
		--border-radius: 3px;
		cursor: pointer;
	}
</style>
