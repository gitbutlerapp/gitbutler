<script lang="ts">
	import { AuthService } from '$lib/auth/authService.svelte';
	import AddSshKeyModal from '$lib/components/AddSshKeyModal.svelte';
	import { featureShowOrganizations, featureShowProjectPage } from '$lib/featureFlags';
	import { SshKeyService, type SshKey } from '$lib/sshKeyService';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { NotificationSettingsService } from '@gitbutler/shared/settings/notificationSettingsService';
	import { getNotificationSettingsInterest } from '@gitbutler/shared/settings/notificationSetttingsPreview.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	const authService = getContext(AuthService);
	const userService = getContext(UserService);
	const appState = getContext(AppState);
	const notificationSettingsService = getContext(NotificationSettingsService);
	const sshKeyService = getContext(SshKeyService);

	const notificationSettings = getNotificationSettingsInterest(
		appState,
		notificationSettingsService
	);

	const user = $derived(userService.user);
	const token = $derived(authService.tokenReadable);

	let updatingReceiveChatMentionEmails = $state(false);
	let updatingReceiveIssueCreationEmails = $state(false);
	let updatingReceiveIssueResolutionEmails = $state(false);
	let updatingReceiveReviewBranchEmails = $state(false);
	let updatingReceiveSignOffEmails = $state(false);
	let updatingName = $state(false);
	let nameValue = $state('');
	let emailValue = $state('');
	let userPicture = $state('');
	let sshKeys = $state<SshKey[]>([]);
	let loadingSshKeys = $state(true);
	let addKeyModal = $state<AddSshKeyModal>();

	$effect(() => {
		if ($user) {
			nameValue = $user.name;
			emailValue = $user.email;
			userPicture = $user.picture;
			loadSshKeys();
		}
	});

	function onPictureChange(e: Event) {
		const target = e.target as HTMLInputElement;
		const file = target.files?.[0];
		const fileTypes = ['image/jpeg', 'image/png'];

		if (file && fileTypes.includes(file.type)) {
			userPicture = URL.createObjectURL(file);
			updateProfilePicture(file);
		} else {
			userPicture = $user?.picture || '';
			// TODO: Add toast notification for invalid file type
		}
	}

	async function updateProfilePicture(file: File) {
		try {
			await userService.updateUser({ picture: file });
		} catch (error) {
			console.error('Failed to update profile picture:', error);
			userPicture = $user?.picture || '';
			// TODO: Add toast notification for error
		}
	}

	async function updateName() {
		if (nameValue === $user?.name) return;
		updatingName = true;
		try {
			await userService.updateUser({ name: nameValue });
		} finally {
			updatingName = false;
		}
	}

	async function updateReceiveChatMentionEmails(value: boolean) {
		updatingReceiveChatMentionEmails = true;
		await notificationSettingsService.updateNotificationSettings({
			receiveChatMentionEmails: value
		});
		updatingReceiveChatMentionEmails = false;
	}

	async function updateReceiveIssueCreationEmails(value: boolean) {
		updatingReceiveIssueCreationEmails = true;
		await notificationSettingsService.updateNotificationSettings({
			receiveIssueCreationEmails: value
		});
		updatingReceiveIssueCreationEmails = false;
	}

	async function updateReceiveIssueResolutionEmails(value: boolean) {
		updatingReceiveIssueResolutionEmails = true;
		await notificationSettingsService.updateNotificationSettings({
			receiveIssueResolutionEmails: value
		});
		updatingReceiveIssueResolutionEmails = false;
	}

	async function updateReceiveReviewBranchEmails(value: boolean) {
		updatingReceiveReviewBranchEmails = true;
		await notificationSettingsService.updateNotificationSettings({
			receiveReviewBranchEmails: value
		});
		updatingReceiveReviewBranchEmails = false;
	}

	async function updateReceiveSignOffEmails(value: boolean) {
		updatingReceiveSignOffEmails = true;
		await notificationSettingsService.updateNotificationSettings({ receiveSignOffEmails: value });
		updatingReceiveSignOffEmails = false;
	}

	async function loadSshKeys() {
		try {
			sshKeys = await sshKeyService.getSshKeys();
		} catch (error) {
			console.error('Failed to load SSH keys:', error);
		} finally {
			loadingSshKeys = false;
		}
	}

	async function deleteSshKey(fingerprint: string) {
		const key = sshKeys.find((k) => k.fingerprint === fingerprint);
		if (!key) return;

		const confirmed = confirm(`Are you sure you want to delete the SSH key "${key.name}"?`);
		if (!confirmed) return;

		try {
			await sshKeyService.deleteSshKey(key.fingerprint);
			sshKeys = sshKeys.filter((key) => key.fingerprint !== fingerprint);
		} catch (error) {
			console.error('Failed to delete SSH key:', error);
		}
	}

	async function onAddKeyModalClose() {
		console.log('Modal closed, refreshing keys...');
		await loadSshKeys();
	}
</script>

<svelte:head>
	<title>GitButler | User</title>
</svelte:head>

<div class="profile-page">
	<div class="content">
		{#if !$token}
			<p>Unauthorized</p>
		{:else if !$user?.id}
			<p>Loading...</p>
		{:else}
			<h1 class="title">Profile</h1>

			<SectionCard>
				<div class="profile-form">
					<label id="profile-picture" class="profile-pic-wrapper" for="picture">
						<input
							type="file"
							id="picture"
							name="picture"
							accept="image/jpeg,image/png"
							class="hidden-input"
							onchange={onPictureChange}
						/>
						{#if userPicture}
							<img class="profile-pic" src={userPicture} alt="" referrerpolicy="no-referrer" />
						{/if}
						<span class="profile-pic__edit-label">Edit</span>
					</label>

					<div class="contact-info">
						<div class="contact-info__fields">
							<div class="info-field">
								<label for="full-name">Full name</label>
								<input
									id="full-name"
									type="text"
									bind:value={nameValue}
									readonly={updatingName}
									onblur={updateName}
									onkeydown={(e) => e.key === 'Enter' && updateName()}
								/>
							</div>
							<div class="info-field">
								<label for="email">Email</label>
								<input id="email" type="email" bind:value={emailValue} readonly={true} />
							</div>
						</div>
					</div>
				</div>
			</SectionCard>

			<h2 class="section-title">Notification settings</h2>

			<Loading loadable={notificationSettings.current}>
				{#snippet children(notificationSettings)}
					<SectionCard>
						<div class="notification-settings">
							<div class="notification-option">
								<label class="checkbox-label" for="receive-chat-mention-emails">
									<input
										type="checkbox"
										id="receive-chat-mention-emails"
										checked={notificationSettings.receiveChatMentionEmails}
										disabled={updatingReceiveChatMentionEmails}
										onchange={() =>
											updateReceiveChatMentionEmails(
												!notificationSettings.receiveChatMentionEmails
											)}
									/>
									<div class="checkbox-content">
										<span class="checkbox-title">Chat message mention emails</span>
										<span class="checkbox-caption">Emails when you are mentioned in a message.</span
										>
									</div>
								</label>
							</div>

							<div class="notification-option">
								<label class="checkbox-label" for="receive-issue-creation-emails">
									<input
										type="checkbox"
										id="receive-issue-creation-emails"
										checked={notificationSettings.receiveIssueCreationEmails}
										disabled={updatingReceiveIssueCreationEmails}
										onchange={() =>
											updateReceiveIssueCreationEmails(
												!notificationSettings.receiveIssueCreationEmails
											)}
									/>
									<div class="checkbox-content">
										<span class="checkbox-title">Issue creation emails</span>
										<span class="checkbox-caption"
											>Emails for new issues created in changes you are involved in.</span
										>
									</div>
								</label>
							</div>

							<div class="notification-option">
								<label class="checkbox-label" for="receive-issue-resolution-emails">
									<input
										type="checkbox"
										id="receive-issue-resolution-emails"
										checked={notificationSettings.receiveIssueResolutionEmails}
										disabled={updatingReceiveIssueResolutionEmails}
										onchange={() =>
											updateReceiveIssueResolutionEmails(
												!notificationSettings.receiveIssueResolutionEmails
											)}
									/>
									<div class="checkbox-content">
										<span class="checkbox-title">Issue status emails</span>
										<span class="checkbox-caption"
											>Emails for status updates of issues in changes you are involved in.</span
										>
									</div>
								</label>
							</div>

							<div class="notification-option">
								<label class="checkbox-label" for="receive-review-branch-emails">
									<input
										type="checkbox"
										id="receive-review-branch-emails"
										checked={notificationSettings.receiveReviewBranchEmails}
										disabled={updatingReceiveReviewBranchEmails}
										onchange={() =>
											updateReceiveReviewBranchEmails(
												!notificationSettings.receiveReviewBranchEmails
											)}
									/>
									<div class="checkbox-content">
										<span class="checkbox-title">Branch version update emails</span>
										<span class="checkbox-caption"
											>Emails when a new review branch version is created.</span
										>
									</div>
								</label>
							</div>

							<div class="notification-option">
								<label class="checkbox-label" for="receive-sign-off-emails">
									<input
										type="checkbox"
										id="receive-sign-off-emails"
										checked={notificationSettings.receiveSignOffEmails}
										disabled={updatingReceiveSignOffEmails}
										onchange={() =>
											updateReceiveSignOffEmails(!notificationSettings.receiveSignOffEmails)}
									/>
									<div class="checkbox-content">
										<span class="checkbox-title">Change status update emails</span>
										<span class="checkbox-caption"
											>Emails for updates on the review status of changes you are involved in.</span
										>
									</div>
								</label>
							</div>
						</div>
					</SectionCard>
				{/snippet}
			</Loading>

			<h2 class="section-title">SSH Keys</h2>

			<SectionCard>
				<div class="ssh-keys">
					{#if loadingSshKeys}
						<div class="loading">Loading SSH keys...</div>
					{:else if sshKeys.length === 0}
						<div class="no-keys">No SSH keys added yet</div>
					{:else}
						{#each sshKeys as key}
							<div class="ssh-key">
								<div class="ssh-key-info">
									<span class="ssh-key-name">{key.name}</span>
									<span class="ssh-key-fingerprint">{key.fingerprint}</span>
								</div>
								<button
									type="button"
									class="delete-button"
									title="Delete key"
									onclick={() => deleteSshKey(key.fingerprint)}>×</button
								>
							</div>
						{/each}
					{/if}

					<button type="button" class="add-key-button" onclick={() => addKeyModal?.show()}>
						<span class="add-key-icon">+</span>
						<span>Add SSH Key</span>
					</button>
				</div>
			</SectionCard>

			<AddSshKeyModal bind:this={addKeyModal} onClose={onAddKeyModalClose} />

			<h2 class="section-title">Experimental settings</h2>

			<SectionCard labelFor="showOrganizations" orientation="row">
				{#snippet title()}Organizations{/snippet}
				{#snippet caption()}
					Organizations are a way of linking together projects.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="showOrganizations"
						checked={$featureShowOrganizations}
						onclick={() => ($featureShowOrganizations = !$featureShowOrganizations)}
					/>
				{/snippet}
			</SectionCard>

			<SectionCard labelFor="showProjectPage" orientation="row">
				{#snippet title()}Project Page{/snippet}
				{#snippet caption()}
					The project page provides an overview of the project.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="showProjectPage"
						checked={$featureShowProjectPage}
						onclick={() => ($featureShowProjectPage = !$featureShowProjectPage)}
					/>
				{/snippet}
			</SectionCard>
		{/if}
	</div>
</div>

<style lang="postcss">
	.profile-page {
		width: 100%;
		min-height: 100vh;
		background-color: var(--clr-bg-2);
	}

	.content {
		padding: 48px 32px;
		display: flex;
		flex-direction: column;
		gap: 16px;
		max-width: 640px;
		width: 100%;
		min-height: 100vh;
		margin: auto;
	}

	.title {
		color: var(--clr-scale-ntrl-0);
		font-size: 24px;
		font-weight: 600;
		align-self: flex-start;
	}

	.section-title {
		color: var(--clr-scale-ntrl-0);
		font-size: 18px;
		font-weight: 600;
		margin-top: 24px;
	}

	.profile-form {
		display: flex;
		gap: 24px;
	}

	.profile-pic-wrapper {
		position: relative;
		width: 100px;
		height: 100px;
		border-radius: var(--radius-m);
		overflow: hidden;
		background-color: var(--clr-scale-pop-70);
		transition: opacity var(--transition-medium);
		cursor: pointer;

		&:hover {
			& .profile-pic__edit-label {
				opacity: 1;
			}

			& .profile-pic {
				opacity: 0.8;
			}
		}
	}

	.profile-pic {
		width: 100%;
		height: 100%;
		object-fit: cover;
		background-color: var(--clr-scale-pop-70);
	}

	.profile-pic__edit-label {
		position: absolute;
		bottom: 8px;
		left: 8px;
		color: var(--clr-core-ntrl-100);
		background-color: var(--clr-scale-ntrl-20);
		padding: 4px 6px;
		border-radius: var(--radius-m);
		opacity: 0;
		transition: opacity var(--transition-medium);
		font-size: 11px;
		font-weight: 600;
	}

	.contact-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.contact-info__fields {
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.info-field {
		display: flex;
		flex-direction: column;
		gap: 4px;

		label {
			color: var(--clr-scale-ntrl-30);
			font-size: 14px;
		}

		input {
			padding: 8px 12px;
			border-radius: var(--radius-m);
			border: 1px solid var(--clr-border-2);
			background-color: var(--clr-bg-1);
			color: var(--clr-scale-ntrl-0);
			font-size: 14px;

			&:read-only {
				opacity: 0.7;
				cursor: not-allowed;
			}

			&:not(:read-only) {
				&:focus {
					border-color: var(--clr-scale-pop-70);
					outline: none;
				}
			}
		}
	}

	.hidden-input {
		cursor: pointer;
		z-index: var(--z-ground);
		position: absolute;
		width: 100%;
		height: 100%;
		opacity: 0;
	}

	.notification-settings {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.notification-option {
		display: flex;
		align-items: flex-start;
	}

	.checkbox-label {
		display: flex;
		gap: 12px;
		cursor: pointer;
		user-select: none;

		input[type='checkbox'] {
			margin-top: 4px;
			width: 16px;
			height: 16px;
			border-radius: var(--radius-s);
			border: 1px solid var(--clr-border-2);
			background-color: var(--clr-bg-1);
			cursor: pointer;

			&:checked {
				background-color: var(--clr-scale-pop-70);
				border-color: var(--clr-scale-pop-70);

				&::after {
					content: '';
					position: absolute;
					left: 5px;
					top: 2px;
					width: 4px;
					height: 8px;
					border: solid white;
					border-width: 0 2px 2px 0;
					transform: rotate(45deg);
				}
			}

			&:disabled {
				opacity: 0.5;
				cursor: not-allowed;
			}
		}
	}

	.checkbox-content {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.checkbox-title {
		color: var(--clr-scale-ntrl-0);
		font-size: 14px;
		font-weight: 500;
	}

	.checkbox-caption {
		color: var(--clr-scale-ntrl-30);
		font-size: 13px;
		line-height: 1.4;
	}

	.ssh-keys {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.ssh-key {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
	}

	.ssh-key-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.ssh-key-name {
		color: var(--clr-scale-ntrl-0);
		font-size: 14px;
		font-weight: 500;
	}

	.ssh-key-fingerprint {
		color: var(--clr-scale-ntrl-30);
		font-size: 13px;
		font-family: monospace;
	}

	.delete-button {
		width: 24px;
		height: 24px;
		border-radius: var(--radius-s);
		border: 1px solid var(--clr-border-2);
		background-color: transparent;
		color: var(--clr-scale-ntrl-30);
		font-size: 18px;
		line-height: 1;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all var(--transition-medium);

		&:hover {
			background-color: var(--clr-scale-ntrl-10);
			border-color: var(--clr-scale-ntrl-30);
			color: var(--clr-scale-ntrl-50);
		}
	}

	.add-key-button {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px;
		border-radius: var(--radius-m);
		border: 1px dashed var(--clr-border-2);
		background-color: transparent;
		color: var(--clr-scale-ntrl-50);
		font-size: 14px;
		cursor: pointer;
		transition: all var(--transition-medium);

		&:hover {
			border-color: var(--clr-scale-pop-70);
			color: var(--clr-scale-pop-70);
		}
	}

	.add-key-icon {
		font-size: 18px;
		line-height: 1;
	}

	.loading,
	.no-keys {
		color: var(--clr-scale-ntrl-30);
		font-size: 14px;
		text-align: center;
		padding: 24px;
	}
</style>
