<script lang="ts">
	import CommitFailedModalContent from '$components/CommitFailedModalContent.svelte';
	import { type GlobalModalState, UI_STATE } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import { Modal } from '@gitbutler/ui';
	import type { ModalProps } from '@gitbutler/ui/Modal.svelte';

	const uiState = inject(UI_STATE);

	type ModalData = {
		state: GlobalModalState;
		props: Omit<ModalProps, 'children'>;
	};

	function mapModalStateToProps(modalState: GlobalModalState | undefined): ModalData | null {
		if (!modalState) return null;

		switch (modalState.type) {
			case 'commit-failed': {
				return {
					state: modalState,
					props: {
						testId: TestId.GlobalModal_CommitFailed,
						closeButton: true,
						width: 540,
						noPadding: true
					}
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
			<CommitFailedModalContent data={modalProps.state} oncloseclick={() => modal?.close()} />
		{/if}
	</Modal>
{/if}
