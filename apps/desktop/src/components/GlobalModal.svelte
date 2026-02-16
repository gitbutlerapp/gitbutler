<script lang="ts">
	import AuthorMissingModalContent from "$components/AuthorMissingModalContent.svelte";
	import AutoCommitModalContent from "$components/AutoCommitModalContent.svelte";
	import CommitFailedModalContent from "$components/CommitFailedModalContent.svelte";
	import GeneralSettingsModalContent from "$components/GeneralSettingsModalContent.svelte";
	import LoginConfirmationModalContent from "$components/LoginConfirmationModalContent.svelte";
	import ProjectSettingsModalContent from "$components/ProjectSettingsModalContent.svelte";
	import { type GlobalModalState, UI_STATE } from "$lib/state/uiState.svelte";
	import { USER_SERVICE } from "$lib/user/userService";
	import { inject } from "@gitbutler/core/context";
	import { Modal, TestId } from "@gitbutler/ui";
	import type { ModalProps } from "@gitbutler/ui";

	const uiState = inject(UI_STATE);
	const userService = inject(USER_SERVICE);
	const incomingUserLogin = $derived(userService.incomingUserLogin);

	type ModalData = {
		state: GlobalModalState;
		props: Omit<ModalProps, "children">;
	};

	function mapModalStateToProps(modalState: GlobalModalState | undefined): ModalData | null {
		if (!modalState) return null;

		switch (modalState.type) {
			case "commit-failed": {
				return {
					state: modalState,
					props: {
						testId: TestId.GlobalModal_CommitFailed,
						closeButton: true,
						width: 540,
						noPadding: true,
					},
				};
			}
			case "author-missing": {
				return {
					state: modalState,
					props: {
						testId: TestId.GlobalModal_AuthorMissing,
						closeButton: true,
						width: 420,
						noPadding: true,
					},
				};
			}
			case "general-settings": {
				return {
					state: modalState,
					props: {
						testId: TestId.GeneralSettingsModal,
						closeButton: true,
						width: 820,
						noPadding: true,
					},
				};
			}
			case "project-settings": {
				return {
					state: modalState,
					props: {
						testId: TestId.ProjectSettingsModal,
						closeButton: true,
						width: 820,
						noPadding: true,
					},
				};
			}
			case "login-confirmation": {
				return {
					state: modalState,
					props: {
						testId: TestId.LoginConfirmationModal,
						closeButton: false,
						width: 360,
						noPadding: true,
						preventCloseOnClickOutside: true,
					},
				};
			}
			case "auto-commit": {
				return {
					state: modalState,
					props: {
						testId: TestId.AutoCommitModal,
						width: 440,
						noPadding: true,
						preventCloseOnClickOutside: true,
					},
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

	function handleModalClose() {
		// If the login confirmation modal is closed without explicit user action (e.g., via ESC),
		// we should reject the incoming user to maintain state consistency.
		// We check if there's still an incoming user to avoid calling reject after accept/reject buttons.
		if (modalProps?.state.type === "login-confirmation") {
			if ($incomingUserLogin) {
				userService.rejectIncomingUser();
			}
		}
		uiState.global.modal.set(undefined);
	}
</script>

{#if modalProps}
	<Modal
		bind:this={modal}
		{...modalProps.props}
		onClose={handleModalClose}
		onSubmit={(close) => close()}
	>
		{#if modalProps.state.type === "commit-failed"}
			<CommitFailedModalContent data={modalProps.state} oncloseclick={() => modal?.close()} />
		{:else if modalProps.state.type === "author-missing"}
			<AuthorMissingModalContent data={modalProps.state} close={closeModal} />
		{:else if modalProps.state.type === "general-settings"}
			<GeneralSettingsModalContent data={modalProps.state} />
		{:else if modalProps.state.type === "project-settings"}
			<ProjectSettingsModalContent data={modalProps.state} />
		{:else if modalProps.state.type === "login-confirmation"}
			<LoginConfirmationModalContent data={modalProps.state} close={closeModal} />
		{:else if modalProps.state.type === "auto-commit"}
			<AutoCommitModalContent data={modalProps.state} close={closeModal} />
		{/if}
	</Modal>
{/if}
