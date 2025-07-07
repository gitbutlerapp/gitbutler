<script lang="ts">
	import FloatingModal from '$lib/floating/FloatingModal.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { type Snippet } from 'svelte';
	import type { SnapPositionName } from '$lib/floating/types';

	interface Props {
		children: Snippet;
		title: string;
		onExitFloatingModeClick: () => void;
	}

	const { children, title, onExitFloatingModeClick }: Props = $props();

	const uiState = getContext(UiState);

	/** Modal dimensions */
	const width = $derived(uiState.global.floatingCommitWidth.current);
	const height = $derived(uiState.global.floatingCommitHeight.current);
	const floatingPosition = $derived(uiState.global.floatingCommitPosition);

	let headerElRef = $state<HTMLDivElement | undefined>(undefined);
</script>

<FloatingModal
	defaults={{
		width,
		minWidth: 420,
		height,
		minHeight: 300,
		snapPosition: floatingPosition.current
	}}
	onUpdateSize={(newWidth, newHeight) => {
		uiState.global.floatingCommitWidth.current = newWidth;
		uiState.global.floatingCommitHeight.current = newHeight;
	}}
	onUpdateSnapPosition={(snapPosition: SnapPositionName) => {
		uiState.global.floatingCommitPosition.current = snapPosition;
	}}
	dragHandleElement={headerElRef}
>
	<div class="modal-header" bind:this={headerElRef}>
		<div class="drag-handle">
			<Icon name="draggable" />
		</div>
		<h4 class="text-14 text-semibold">
			{title}
		</h4>
	</div>

	<div class="modal-content">
		{@render children()}
	</div>
</FloatingModal>

<button class="exit-floating-mode" type="button" onclick={onExitFloatingModeClick}>
	<span class="text-12 text-semibold underline-dotted">Exit floating mode</span>
</button>

<style>
	.exit-floating-mode {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		padding: 12px;
		gap: 8px;
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
		cursor: pointer;
		transition: background-color 0.2s ease-in-out;

		&:hover {
			background-color: var(--clr-bg-1);
		}
	}

	.modal-header {
		display: flex;
		align-items: center;
		padding: 12px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
		background: var(--clr-bg-2);
		cursor: grab;
	}

	.modal-header h4 {
		flex: 1;
	}

	.drag-handle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		color: var(--clr-text-2);
	}

	.modal-content {
		display: flex;
		flex-direction: column;
		height: 100%;
		padding: 16px;
		overflow: auto;
	}
</style>
