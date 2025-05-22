<script lang="ts">
	import CommitFailedModalContent from '$components/CommitFailedModalContent.svelte';
	import { UiState, type GlobalModalState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal, { type ModalProps } from '@gitbutler/ui/Modal.svelte';

	const [uiState] = inject(UiState);

	type ModalData = {
		state: GlobalModalState;
		props: Omit<ModalProps, 'children'>;

		actionButtonLabel: string;
	};

	function mapModalStateToProps(modalState: GlobalModalState | undefined): ModalData | null {
		if (!modalState) return null;

		switch (modalState.type) {
			case 'commit-failed': {
				return {
					state: modalState,
					props: {
						testId: TestId.GlobalModal_CommitFailed,
						title: modalState.newCommitId
							? 'Some changes were not committed'
							: 'Failed to create commit',
						type: modalState.newCommitId ? 'warning' : 'error',
						closeButton: true,
						width: 'large'
					},
					actionButtonLabel: modalState.newCommitId ? 'Oh ok' : 'Well, that sucks'
				};
			}
		}
	}

	const modalProps = $derived(mapModalStateToProps(uiState.global.modal.current));

	let modal = $state<Modal>();

	$effect(() => {
		if (modalProps) {
			modal?.show();
		}
	});
</script>

{#if modalProps}
	<Modal
		bind:this={modal}
		{...modalProps.props}
		onClose={() => uiState.global.modal.set(undefined)}
		onSubmit={(close) => close()}
	>
		{#if modalProps.state.type === 'commit-failed'}
			<CommitFailedModalContent data={modalProps.state} />
		{/if}

		{#snippet controls()}
			<Button testId={TestId.GlobalModalActionButton} style="neutral" type="submit"
				>{modalProps.actionButtonLabel}</Button
			>
		{/snippet}
	</Modal>
{/if}
