<script lang="ts">
	import AuthorMissingModalContent from '$components/AuthorMissingModalContent.svelte';
	import CommitFailedModalContent from '$components/CommitFailedModalContent.svelte';
	import GeneralSettingsModalContent from '$components/GeneralSettingsModalContent.svelte';
	import ProjectSettingsModalContent from '$components/ProjectSettingsModalContent.svelte';
	import { type GlobalModalState, UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Modal, TestId } from '@gitbutler/ui';
	import type { ModalProps } from '@gitbutler/ui';

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
			case 'author-missing': {
				return {
					state: modalState,
					props: {
						testId: TestId.GlobalModal_AuthorMissing,
						closeButton: true,
						width: 420,
						noPadding: true
					}
				};
			}
			case 'general-settings': {
				return {
					state: modalState,
					props: {
						testId: 'general-settings-modal',
						closeButton: true,
						width: 1000,
						noPadding: true
					}
				};
			}
			case 'project-settings': {
				return {
					state: modalState,
					props: {
						testId: 'project-settings-modal',
						closeButton: true,
						width: 1000,
						noPadding: true
					}
				};
			}
		}
	}

	const modalProps = $derived(mapModalStateToProps(uiState.global.modal.current));

	let modal = $state<Modal>();

	// Handle modal showing/hiding with proper timing
	$effect(() => {
		if (!modal) return;

		if (modalProps) {
			modal.show();
		} else {
			modal.close();
		}
	});

	function closeModal() {
		modal?.close();
	}
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
		{:else if modalProps.state.type === 'author-missing'}
			<AuthorMissingModalContent data={modalProps.state} close={closeModal} />
		{:else if modalProps.state.type === 'general-settings'}
			<GeneralSettingsModalContent data={modalProps.state} close={closeModal} />
		{:else if modalProps.state.type === 'project-settings'}
			<ProjectSettingsModalContent data={modalProps.state} close={closeModal} />
		{/if}
	</Modal>
{/if}
