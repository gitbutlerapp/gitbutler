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
		width: 100%;
		padding: 4px 2px 4px 6px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m) var(--radius-m) 0 0;
		background-color: var(--clr-theme-ntrl-soft);

		--hover-color: oklch(from var(--clr-scale-ntrl-70) l c h / 0.5);
		--hover-color-on-target: oklch(from var(--clr-scale-pop-70) l c h / 0.7);

		& .draggable-icon {
			display: flex;
			cursor: grab;
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
			border-radius: var(--radius-s);
			color: var(--clr-text-2);
			opacity: 0.7;
			transition: background-color var(--transition-fast);

			&:hover {
				background-color: var(--clr-theme-ntrl-soft-hover);
				color: var(--clr-text-2);
				opacity: 1;
			}
		}

		& .target-btn {
			display: flex;
			align-items: center;
			padding: 1px 5px 1px 3px;
			gap: 4px;
			border-radius: var(--radius-s);

			transition: background-color var(--transition-fast);

			&:not(:disabled) {
				color: var(--clr-theme-ntrl-element);
				cursor: pointer;

				&:hover {
					background-color: var(--clr-theme-ntrl-soft-hover);
				}
			}

			&:disabled {
				color: var(--clr-theme-pop-on-soft);
				pointer-events: none;
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
				background-color: var(--hover-color-on-target);
				color: var(--clr-theme-pop-on-soft);
				opacity: 0.7;
			}
		}
	}
</style>
