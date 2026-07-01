<script lang="ts">
	import CommitFailedModalContent from "$components/commit/CommitFailedModalContent.svelte";
	import LoginConfirmationModalContent from "$components/onboarding/LoginConfirmationModalContent.svelte";
	import AuthorMissingModalContent from "$components/settings/AuthorMissingModalContent.svelte";
	import GeneralSettingsModalContent from "$components/settings/GeneralSettingsModalContent.svelte";
	import ProjectSettingsModalContent from "$components/settings/ProjectSettingsModalContent.svelte";
	import { type GlobalModalState, UI_STATE } from "$lib/state/uiState.svelte";
	import { USER_SERVICE } from "$lib/user/userService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Modal, TestId } from "@gitbutler/ui";
	import type { ModalProps } from "@gitbutler/ui";

	const uiState = inject(UI_STATE);
	const userService = inject(USER_SERVICE);

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
		}
	}

	const modalProps = $derived(mapModalStateToProps(uiState.global.modal.current));

	// Svelte 5 can propagate prop changes into the child block before the outer
	// {#if} re-evaluates and unmounts it, causing crashes like
	// "undefined is not an object (evaluating 'data.projectId')". stableModalData
	// latches the last non-null modalProps so the children always see valid data;
	// the {#if modalProps && stableModalData} gate handles visibility.
	let stableModalData = $state<ModalData | null>(null);
	$effect.pre(() => {
		if (modalProps !== null) {
			stableModalData = modalProps;
		}
	});

	let modal = $state<Modal>();

	// Show the modal whenever modalProps becomes truthy.
	// When modalProps becomes falsy the {#if} block unmounts the Modal,
	// so we only need to handle the "show" direction here.
	$effect(() => {
		if (modal && modalProps) {
			modal.show();
		}
	});

	function handleModalClose() {
		// If the login confirmation modal is closed without explicit user action (e.g., via ESC),
		// we should reject the incoming user to maintain state consistency.
		// We check if there's still an incoming user to avoid calling reject after accept/reject buttons.
		if (stableModalData?.state.type === "login-confirmation") {
			if (userService.incomingUserLogin) {
				userService.rejectIncomingUser();
			}
		}
		uiState.global.modal.set(undefined);
	}

	/**
	 * Close the modal via the Modal component's own close() method so that
	 * the portalled DOM is properly cleaned up with its closing animation.
	 * Falls back to clearing state directly if the Modal ref is unavailable
	 * (e.g. due to an unmount race condition).
	 */
	function closeModal() {
		if (modal) {
			modal.close();
		} else {
			handleModalClose();
		}
	}
</script>

{#if modalProps && stableModalData}
	<Modal
		bind:this={modal}
		{...stableModalData.props}
		onClose={handleModalClose}
		onSubmit={(close) => close()}
	>
		{#if stableModalData.state.type === "commit-failed"}
			<CommitFailedModalContent data={stableModalData.state} oncloseclick={closeModal} />
		{:else if stableModalData.state.type === "author-missing"}
			<AuthorMissingModalContent data={stableModalData.state} close={closeModal} />
		{:else if stableModalData.state.type === "general-settings"}
			<GeneralSettingsModalContent data={stableModalData.state} />
		{:else if stableModalData.state.type === "project-settings"}
			<ProjectSettingsModalContent data={stableModalData.state} />
		{:else if stableModalData.state.type === "login-confirmation"}
			<LoginConfirmationModalContent data={stableModalData.state} close={closeModal} />
		{/if}
	</Modal>
{/if}
