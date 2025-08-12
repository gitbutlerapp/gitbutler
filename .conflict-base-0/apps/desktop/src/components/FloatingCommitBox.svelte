<script lang="ts">
	import FloatingModal from '$lib/floating/FloatingModal.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Icon } from '@gitbutler/ui';

	import { type Snippet } from 'svelte';
	import type { SnapPositionName } from '$lib/floating/types';

	interface Props {
		children: Snippet;
		title: string;
		onExitFloatingModeClick: () => void;
	}

	const { children, title, onExitFloatingModeClick }: Props = $props();

	const uiState = inject(UI_STATE);

	const { width, height } = $derived(uiState.global.floatingBoxSize.current);
	const snapPosition = $derived(uiState.global.floatingBoxPosition.current);

	let headerElRef = $state<HTMLDivElement | undefined>(undefined);
</script>

<FloatingModal
	defaults={{
		width,
		height,
		snapPosition,
		minWidth: 420,
		minHeight: 280
	}}
	onUpdateSize={(newWidth, newHeight) => {
		uiState.global.floatingBoxSize.set({ width: newWidth, height: newHeight });
	}}
	onUpdateSnapPosition={(snapPosition: SnapPositionName) => {
		uiState.global.floatingBoxPosition.set(snapPosition);
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
