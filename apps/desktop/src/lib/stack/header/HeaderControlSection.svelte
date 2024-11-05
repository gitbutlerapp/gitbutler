<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';

	interface Props {
		isDefault: boolean;
		onDefaultSet: () => void;
	}

	const { isDefault, onDefaultSet }: Props = $props();
</script>

<div class="header-control" class:is-default={isDefault} data-drag-handle>
	<div class="draggable-icon" data-drag-handle>
		<Icon name="draggable" />
	</div>

	<button type="button" class="target-btn" onclick={onDefaultSet} disabled={isDefault}>
		<div class="target-btn__icon">
			<Icon name="target" />
		</div>
		<span class="text-11 text-semibold">
			{#if isDefault}
				Default lane
			{:else}
				Set as default
			{/if}
		</span>
	</button>
</div>

<style lang="postcss">
	.header-control {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 3px 2px 3px 6px;
		background-color: var(--clr-theme-ntrl-soft);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m) var(--radius-m) 0 0;
		width: 100%;

		& .draggable-icon {
			cursor: grab;
			display: flex;
			color: var(--clr-scale-ntrl-0);
			opacity: 0.3;
			transition: opacity var(--transition-fast);

			&:hover {
				opacity: 0.6;
			}
		}

		& .target-btn {
			display: flex;
			align-items: center;
			gap: 4px;
			padding: 1px 5px 1px 3px;
			border-radius: var(--radius-s);
			border: 1px solid transparent;

			transition:
				background-color var(--transition-fast),
				border-color var(--transition-fast);

			&:not(:disabled) {
				cursor: pointer;
				color: var(--clr-theme-ntrl-element);

				&:hover {
					background-color: var(--clr-theme-ntrl-soft-hover);
					border-color: var(--clr-border-2);
				}
			}

			&:disabled {
				pointer-events: none;
				color: var(--clr-theme-pop-on-soft);
			}
		}

		& .target-btn__icon {
			display: flex;
			opacity: 0.5;
		}
	}

	/* MODIFIERS */
	.is-default {
		background-color: var(--clr-theme-pop-soft);
	}
</style>
