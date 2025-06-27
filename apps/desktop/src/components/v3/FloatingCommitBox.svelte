<script lang="ts">
	import FloatingModal from '$lib/floating/FloatingModal.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { type Snippet } from 'svelte';
	import type { SnapPositionName } from '$lib/floating/types';

	interface Props {
		children: Snippet;
		header: Snippet;
		onExitFloatingModeClick: () => void;
	}

	const { children, header, onExitFloatingModeClick }: Props = $props();

	const uiState = getContext(UiState);

	/** Modal dimensions */
	const width = $derived(uiState.global.floatingCommitWidth.current);
	const height = $derived(uiState.global.floatingCommitHeight.current);
	const floatingPosition = $derived(uiState.global.floatingCommitPosition);
</script>

<FloatingModal
	defaults={{
		width,
		height,
		snapPosition: floatingPosition.current
	}}
	{header}
	onUpdateSize={(newWidth, newHeight) => {
		uiState.global.floatingCommitWidth.current = newWidth;
		uiState.global.floatingCommitHeight.current = newHeight;
	}}
	onUpdateSnapPosition={(snapPosition: SnapPositionName) => {
		uiState.global.floatingCommitPosition.current = snapPosition;
	}}
>
	{@render children()}
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
</style>
