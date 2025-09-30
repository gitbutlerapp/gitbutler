<script lang="ts">
	interface Props {
		label?: string;
		showArrow?: boolean;
		disabled?: boolean;
		reverseDirection?: boolean;
		onclick: () => void;
	}

	const { label, showArrow = true, disabled = false, reverseDirection, onclick }: Props = $props();
</script>

<button
	type="button"
	class:reverse-direction={reverseDirection}
	class="arrow-button"
	{onclick}
	{disabled}
	aria-label={label}
>
	{#if label}
		<span class="arrow-button__text">{label}</span>
	{/if}

	{#if showArrow}
		<div class="arrow-button__icon">
			<div class="arrow-button__icon-tail"></div>
			<svg
				width="13"
				height="43"
				viewBox="0 0 13 43"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path d="M1.17773 1L11.5004 21.5L1.17773 42" stroke="currentColor" stroke-width="1.5" />
			</svg>
		</div>
	{/if}
</button>

<style>
	.arrow-button {
		display: flex;
		align-items: center;
		padding: 10px 14px 10px 16px;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: 60px;
		cursor: pointer;
		transition: padding 0.15s ease;

		&:hover .arrow-button__icon-tail {
			width: 36px;
		}

		&:disabled {
			padding: 10px 10px 10px 16px;
			background-color: var(--clr-bg-3);
			color: var(--clr-text-2);
			cursor: not-allowed;
			opacity: 0.5;

			.arrow-button__icon-tail {
				width: 14px;
			}
		}

		&.reverse-direction {
			flex-direction: row-reverse;
			padding: 10px 16px 10px 14px;

			.arrow-button__icon {
				transform: rotate(180deg);
			}
		}
	}

	.arrow-button__text {
		font-size: 40px;
		line-height: 1;
		font-family: var(--fontfamily-accent);
	}

	.arrow-button__icon {
		display: flex;
		align-items: center;
	}

	.arrow-button__icon-tail {
		width: 20px;
		height: 0.094rem;
		margin-right: -10px;
		background-color: currentColor;
		transition: width 0.15s ease;
	}
</style>
