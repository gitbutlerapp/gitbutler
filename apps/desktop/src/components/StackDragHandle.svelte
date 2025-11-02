<script lang="ts">
	import CollapseStackButton from '$components/CollapseStackButton.svelte';
	import { Icon } from '@gitbutler/ui';

	type Props = {
		stackId?: string;
		projectId: string;
		disabled?: boolean;
		onOptionsClick?: () => void;
	};

	let { stackId, projectId, disabled = false, onOptionsClick }: Props = $props();
</script>

<div class="drag-handle-row" data-remove-from-panning data-drag-handle draggable="true">
	<CollapseStackButton {stackId} {projectId} {disabled} />

	<div class="drag-handle-icon">
		<Icon name="draggable-wide" />
	</div>

	<button type="button" class="stack-options-button" onclick={onOptionsClick}>
		<Icon name="kebab" />
	</button>
</div>

<style lang="postcss">
	.drag-handle-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 16px;
		height: 24px;
		margin: 0 -3px;
		color: var(--clr-text-3);
		cursor: grab;
		transition:
			height var(--transition-medium),
			color var(--transition-fast);
	}

	.drag-handle-icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 18px;
		height: 10px;
		padding: 2px;
		border-radius: 3px;
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
		pointer-events: none;
	}

	.stack-options-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}
	}
</style>
