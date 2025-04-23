<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	interface Props {
		isDefault: boolean;
		onDefaultSet: () => void;
		onCollapseButtonClick: () => void;
	}

	const { isDefault, onDefaultSet, onCollapseButtonClick }: Props = $props();
</script>

<div class="header-control" class:is-default={isDefault} data-drag-handle>
	<div class="actions">
		<div class="draggable-icon" data-drag-handle>
			<Icon name="draggable" />
		</div>

		<Tooltip text="Collapse lane">
			<button type="button" class="action-button" onclick={onCollapseButtonClick}>
				<Icon name="fold-lane" />
			</button>
		</Tooltip>
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
		padding: 4px 2px 4px 6px;
		background-color: var(--clr-theme-ntrl-soft);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m) var(--radius-m) 0 0;
		width: 100%;

		--hover-color: oklch(from var(--clr-scale-ntrl-70) l c h / 0.5);
		--hover-color-on-target: oklch(from var(--clr-scale-pop-70) l c h / 0.7);

		& .draggable-icon {
			cursor: grab;
			display: flex;
			opacity: 0.5;
		}

		& .actions {
			display: flex;
			align-items: center;
			gap: 4px;
		}

		& .action-button {
			display: flex;
			align-items: center;
			padding: 2px;
			color: var(--clr-text-2);
			border-radius: var(--radius-s);
			opacity: 0.7;
			transition: background-color var(--transition-fast);

			&:hover {
				color: var(--clr-text-2);
				opacity: 1;
				background-color: var(--clr-theme-ntrl-soft-hover);
			}
		}

		& .target-btn {
			display: flex;
			align-items: center;
			gap: 4px;
			padding: 1px 5px 1px 3px;
			border-radius: var(--radius-s);

			transition: background-color var(--transition-fast);

			&:not(:disabled) {
				cursor: pointer;
				color: var(--clr-theme-ntrl-element);

				&:hover {
					background-color: var(--clr-theme-ntrl-soft-hover);
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

		& .draggable-icon {
			color: var(--clr-theme-pop-on-soft);
		}

		& .action-button {
			color: var(--clr-theme-pop-on-soft);
			opacity: 0.5;

			&:hover {
				color: var(--clr-theme-pop-on-soft);
				opacity: 0.7;
				background-color: var(--hover-color-on-target);
			}
		}
	}
</style>
