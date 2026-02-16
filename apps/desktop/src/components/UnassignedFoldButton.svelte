<script lang="ts">
	interface Props {
		active: boolean;
		onclick: () => void;
	}

	const { active, onclick }: Props = $props();
</script>

<button type="button" class="fold-btn" class:active {onclick} aria-label="Toggle fold">
	<div class="fold-icon__frame"></div>
</button>

<style lang="postcss">
	.fold-btn {
		--border-width: 0.094rem; /* 1.5px */
		--border-color: var(--clr-text-2);
		--radius: 4px;
		--transition: width 0.15s ease, color var(--transition-fast);

		position: relative;
		flex-shrink: 0;
		width: 20px;
		height: 20px;
		border-radius: var(--radius-m);

		&:not(.active):hover {
			--border-color: var(--clr-text-2);

			.fold-icon__frame::before {
				width: 60%;
			}
		}

		&.active {
			.fold-icon__frame::before {
				width: 60%;
				background-color: var(--border-color);
				opacity: 0.5;
			}

			&:hover {
				--border-color: var(--clr-text-2);

				.fold-icon__frame::before {
					width: 50%;
					opacity: 1;
				}
			}
		}
	}

	.fold-icon__frame {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 14px;
		height: 12px;
		overflow: hidden;
		transform: translate(-50%, -50%);
		border-radius: var(--radius);
		box-shadow: inset 0 0 0 var(--border-width) var(--border-color);
		transition: var(--transition);

		&::before {
			position: absolute;
			top: 0;
			right: 0;
			width: 45%;
			height: 100%;
			border: var(--border-width) solid var(--border-color);
			content: '';
			transition:
				var(--transition),
				opacity var(--transition-fast);
		}
	}
</style>
