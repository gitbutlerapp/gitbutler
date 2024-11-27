<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		isLast?: boolean;
		isFirst?: boolean;
		onclick?: () => void;
	}

	const { isLast = false, isFirst = false, onclick }: Props = $props();
</script>

<div class="line-container" class:is-last={isLast} class:is-first={isFirst}>
	<div class="barnch-plus-btn">
		<Button
			style="ghost"
			outline
			solidBackground
			icon="plus-small"
			size="tag"
			width={26}
			tooltip="Insert empty commit"
			helpShowDelay={500}
			onclick={() => onclick?.()}
		/>
	</div>
</div>

<style lang="postcss">
	.line-container {
		--height: 14px;
		--container-margin: calc(var(--height) / 2 * -1);

		position: relative;
		width: 100%;
		height: var(--height);
		z-index: var(--z-lifted);
		margin-top: var(--container-margin);
		margin-bottom: var(--container-margin);

		/* background-color: rgba(235, 167, 78, 0.159); */

		&:hover {
			& .barnch-plus-btn {
				pointer-events: all;
				opacity: 1;
				transform: translateY(-50%) scale(1);
			}
		}

		&:not(.is-last, .is-first) {
			&:before {
				pointer-events: none;
				content: '';
				position: absolute;
				top: 50%;
				left: 0;
				width: 100%;
				height: 1px;
				background-color: var(--clr-border-2);
				transform: translateY(-50%);
				opacity: 0;
				transition: opacity var(--transition-fast);
			}

			&:hover {
				&:before {
					opacity: 1;
				}
			}
		}
	}

	.barnch-plus-btn {
		position: absolute;
		top: 50%;
		right: 24px;
		width: fit-content;
		display: flex;
		align-items: center;
		transform: translateY(-45%) scale(0.8);
		opacity: 0;
		pointer-events: none;
		transition:
			opacity var(--transition-fast),
			transform var(--transition-medium);
	}
</style>
