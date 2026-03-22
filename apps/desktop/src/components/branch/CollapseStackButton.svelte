<script lang="ts">
	import { Tooltip } from "@gitbutler/ui";

	interface Props {
		disabled?: boolean;
		isFolded?: boolean;
		onClick?: () => void;
	}

	let { disabled, isFolded, onClick }: Props = $props();

	const label = $derived(isFolded ? "Expand stack" : "Collapse stack");
</script>

<Tooltip text={label}>
	<button
		class="collapse-button"
		class:isFolded
		type="button"
		aria-label={label}
		onclick={onClick}
		{disabled}
	>
		<div class="collapse-icon">
			<div class="collapse-icon__border"></div>
			<div class="collapse-icon__lane"></div>
		</div>
	</button>
</Tooltip>

<style lang="postcss">
	.collapse-button {
		display: flex;
		position: relative;
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
				background-color: currentColor;
			}
		}

		&:disabled {
			opacity: 0.5;
			pointer-events: none;
		}

		&.isFolded {
			.collapse-icon__lane {
				background-color: currentColor;
			}
		}

		&.isFolded:hover {
			cursor: pointer;
			.collapse-icon__lane {
				background-color: transparent;
			}
		}

		&:after {
			position: absolute;
			top: 50%;
			left: 50%;
			width: 18px;
			height: 14px;
			transform: translate(-50%, -50%);
			background-color: var(--clr-bg-2);
			content: "";
		}
	}

	.collapse-icon {
		z-index: var(--z-ground);
		position: relative;
		width: 15px;
		height: 10px;
		cursor: pointer;
	}

	.collapse-icon__border {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		border: 1.5px solid currentColor;
		border-radius: 3px;
	}

	.collapse-icon__lane {
		position: absolute;
		top: 0;
		left: 0;
		width: 7px;
		height: 100%;
		border: 1.5px solid currentColor;
		border-radius: 3px;
		transition: background-color var(--transition-fast);
	}
</style>
