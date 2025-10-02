<script lang="ts">
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import AddSshKeyModal from '$lib/components/AddSshKeyModal.svelte';
	import { featureShowOrganizations, featureShowProjectPage } from '$lib/featureFlags';
	import { SSH_KEY_SERVICE, type SshKey } from '$lib/sshKeyService';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { getRecentlyPushedProjects } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';
	import { NOTIFICATION_SETTINGS_SERVICE } from '@gitbutler/shared/settings/notificationSettingsService';
	import { getNotificationSettingsInterest } from '@gitbutler/shared/settings/notificationSetttingsPreview.svelte';
	import { Button, SectionCard, Toggle } from '@gitbutler/ui';
	import { env } from '$env/dynamic/public';

	const authService = inject(AUTH_SERVICE);
	const userService = inject(USER_SERVICE);
	const appState = inject(APP_STATE);
	const notificationSettingsService = inject(NOTIFICATION_SETTINGS_SERVICE);
	const sshKeyService = inject(SSH_KEY_SERVICE);

	const notificationSettings = getNotificationSettingsInterest(
		appState,
		notificationSettingsService
	);
	const recentProjects = getRecentlyPushedProjects();

	const user = $derived(userService.user);
	const token = $derived(authService.tokenReadable);

	let updatingReceiveChatMentionEmails = $state(false);
	let updatingReceiveChatReplyEmails = $state(false);
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
	let sshKeyToken = $state('');
	let showSshKeyTokenModal = $state(false);
	let generatingSshToken = $state(false);

	// New state variables for additional profile fields
	let websiteValue = $state('');
	let twitterValue = $state('');
	let blueskyValue = $state('');
	let locationValue = $state('');
	let timezoneValue = $state('');
	let emailShareValue = $state(false);
	let updatingAdditionalInfo = $state(false);

	$effect(() => {
		if ($user) {
			nameValue = $user.name;
			emailValue = $user.email;
			userPicture = $user.picture;
			// Initialize additional profile fields if they exist on the user object
			websiteValue = $user.website || '';
			twitterValue = $user.twitter || '';
			blueskyValue = $user.bluesky || '';
			locationValue = $user.location || '';
			timezoneValue = $user.timezone || '';
			emailShareValue = $user.emailShare || false;
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

	async function updateAdditionalInfo() {
		updatingAdditionalInfo = true;
		try {
			await userService.updateUser({
				website: websiteValue,
				twitter: twitterValue,
				bluesky: blueskyValue,
				location: locationValue,
				timezone: timezoneValue,
				emailShare: emailShareValue
			});
		} finally {
			updatingAdditionalInfo = false;
		}
	}

	async function updateReceiveChatMentionEmails(value: boolean) {
		updatingReceiveChatMentionEmails = true;
		await notificationSettingsService.updateNotificationSettings({
			receiveChatMentionEmails: value
		});
		updatingReceiveChatMentionEmails = false;
	}

	async function updateReceiveChatReplyEmails(value: boolean) {
		updatingReceiveChatReplyEmails = true;
		await notificationSettingsService.updateNotificationSettings({
			receiveChatReplyEmails: value
		});
		updatingReceiveChatReplyEmails = false;
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
		await loadSshKeys();
	}

	async function generateSshKeyToken() {
		generatingSshToken = true;
		try {
			const formData = new FormData();
			formData.append('generate_ssh_token', 'true');

			const updatedUser = await userService.updateUser({
				generate_ssh_token: true
			});

			if (updatedUser && updatedUser.ssh_key_token) {
				sshKeyToken = updatedUser.ssh_key_token;
				showSshKeyTokenModal = true;
			} else {
				console.error('Failed to generate SSH key token: No token returned');
			}
		} catch (error) {
			console.error('Failed to generate SSH key token:', error);
		} finally {
			generatingSshToken = false;
		}
	}

	function closeSshKeyTokenModal() {
		showSshKeyTokenModal = false;
		sshKeyToken = '';
	}

	function handleModalClick(e: Event) {
		e.stopPropagation();
	}
</script>

<svelte:head>
	<title>GitButler | User</title>
</svelte:head>

<div class="profile-page">
	<div class="content">
		{#if !$token || !$user?.id}
			<p>Loading...</p>
		{:else}
			<h1 class="title">My Preferences</h1>

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

			{#if $user?.supporter}
				<div class="supporter-card">
					<div class="supporter-card__content">
						<h3 class="supporter-card__title">🎉 Thank you for being a supporter!</h3>
						<p class="supporter-card__description">
							Your support helps us build a better GitButler. We appreciate your contribution.
						</p>
						<Button
							style="pop"
							onclick={() => (window.location.href = `${env.PUBLIC_APP_HOST}supporter/portal`)}
						>
							Manage your subscription
						</Button>
					</div>
				</div>
			{/if}

			<h2 class="section-title">Contact Info</h2>
			<SectionCard>
				<div class="additional-info">
					<div class="additional-info__fields">
						<div class="info-field">
							<label for="website">Website</label>
							<input
								id="website"
								type="url"
								placeholder="https://example.com"
								bind:value={websiteValue}
								readonly={updatingAdditionalInfo}
							/>
						</div>

						<div class="info-field">
							<label for="twitter">Twitter</label>
							<input
								id="twitter"
								type="text"
								placeholder="@username"
								bind:value={twitterValue}
								readonly={updatingAdditionalInfo}
							/>
						</div>

						<div class="info-field">
							<label for="bluesky">Bluesky</label>
							<input
								id="bluesky"
								type="text"
								placeholder="@handle.bsky.social"
								bind:value={blueskyValue}
								readonly={updatingAdditionalInfo}
							/>
						</div>

						<div class="info-field">
							<label for="location">Location</label>
							<input
								id="location"
								type="text"
								placeholder="City, Country"
								bind:value={locationValue}
								readonly={updatingAdditionalInfo}
							/>
						</div>

						<div class="notification-option">
							<label class="checkbox-label" for="email-share">
								<input
									type="checkbox"
									id="email-share"
									checked={emailShareValue}
									disabled={updatingAdditionalInfo}
									onchange={() => (emailShareValue = !emailShareValue)}
								/>
								<div class="checkbox-content">
									<span class="checkbox-title">Share my email</span>
									<span class="checkbox-caption">
										Allow other users to see your email address.
									</span>
								</div>
							</label>
						</div>

						<button
							type="button"
							class="save-button"
							disabled={updatingAdditionalInfo}
							onclick={updateAdditionalInfo}
						>
							{updatingAdditionalInfo ? 'Saving...' : 'Save Changes'}
						</button>
					</div>
				</div>
			</SectionCard>

			{#if recentProjects.current.length > 0}
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
											<span class="checkbox-caption"
												>Emails when you are mentioned in a message.</span
											>
										</div>
									</label>
								</div>

								<div class="notification-option">
									<label class="checkbox-label" for="receive-chat-reply-emails">
										<input
											type="checkbox"
											id="receive-chat-reply-emails"
											checked={notificationSettings.receiveChatReplyEmails}
											disabled={updatingReceiveChatReplyEmails}
											onchange={() =>
												updateReceiveChatReplyEmails(!notificationSettings.receiveChatReplyEmails)}
										/>
										<div class="checkbox-content">
											<span class="checkbox-title">Chat message reply emails</span>
											<span class="checkbox-caption"
												>Emails when you receive a reply to a chat message.</span
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
							<span>Upload SSH Public Key</span>
						</button>

						<button
							type="button"
							class="add-key-button"
							onclick={generateSshKeyToken}
							disabled={generatingSshToken}
						>
							<span class="add-key-icon">+</span>
							<span>{generatingSshToken ? 'Generating...' : 'Add Key via SSH'}</span>
						</button>
					</div>
				</SectionCard>

				<AddSshKeyModal bind:this={addKeyModal} onClose={onAddKeyModalClose} />

				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				{#if showSshKeyTokenModal}
					<div class="ssh-token-modal-backdrop" onclick={closeSshKeyTokenModal}>
						<div class="ssh-token-modal" onclick={handleModalClick}>
							<h3 class="ssh-token-modal__title">Add Your SSH Key</h3>
							<p class="ssh-token-modal__description">
								Run the following command in your terminal to add your SSH key to GitButler:
							</p>
							<div class="ssh-token-modal__code">
								<code>ssh git@ssh.gitbutler.com add/{sshKeyToken}</code>
								<button
									type="button"
									class="ssh-token-modal__copy-button"
									onclick={() => {
										navigator.clipboard.writeText(`ssh git@ssh.gitbutler.com add/${sshKeyToken}`);
									}}
								>
									Copy
								</button>
							</div>
							<p class="ssh-token-modal__note">This token will expire after use or in 5 minutes.</p>
							<div class="ssh-token-modal__controls">
								<button
									type="button"
									class="ssh-token-modal__close-button"
									onclick={closeSshKeyTokenModal}>Close</button
								>
							</div>
						</div>
					</div>
				{/if}

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
					{#snippet title()}User / Organization / Project Pages{/snippet}
					{#snippet caption()}
						This will show the stub landing pages for orgs, users and projects.
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
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 640px;
		min-height: 100vh;
		margin: auto;
		padding: 48px 32px;
		gap: 16px;
	}

	.title {
		align-self: flex-start;
		color: var(--clr-scale-ntrl-0);
		font-weight: 600;
		font-size: 24px;
	}

	.section-title {
		margin-top: 24px;
		color: var(--clr-scale-ntrl-0);
		font-weight: 600;
		font-size: 18px;
	}

	.profile-form {
		display: flex;
		gap: 24px;
	}

	.supporter-card__content {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 12px;
		border: 1px solid var(--clr-scale-pop-80);
		border-radius: var(--radius-m);
		background-color: var(--clr-scale-pop-95);
	}

	.supporter-card__title {
		color: var(--clr-scale-pop-40);
		font-weight: 600;
		font-size: 18px;
	}

	.supporter-card__description {
		color: var(--clr-scale-ntrl-0);
		font-size: 14px;
		line-height: 1.5;
	}

	.profile-pic-wrapper {
		position: relative;
		width: 100px;
		height: 100px;
		overflow: hidden;
		border-radius: var(--radius-m);
		background-color: var(--clr-scale-pop-70);
		cursor: pointer;
		transition: opacity var(--transition-medium);

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
		padding: 4px 6px;
		border-radius: var(--radius-m);
		background-color: var(--clr-scale-ntrl-20);
		color: var(--clr-core-ntrl-100);
		font-weight: 600;
		font-size: 11px;
		opacity: 0;
		transition: opacity var(--transition-medium);
	}

	.contact-info {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 20px;
	}

	.contact-info__fields {
		display: flex;
		flex-direction: column;
		width: 100%;
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
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m);
			background-color: var(--clr-bg-1);
			color: var(--clr-scale-ntrl-0);
			font-size: 14px;

			&:read-only {
				cursor: not-allowed;
				opacity: 0.7;
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
		z-index: var(--z-ground);
		position: absolute;
		width: 100%;
		height: 100%;
		cursor: pointer;
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
			width: 16px;
			height: 16px;
			margin-top: 4px;
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-s);
			background-color: var(--clr-bg-1);
			cursor: pointer;

			&:checked {
				border-color: var(--clr-scale-pop-70);
				background-color: var(--clr-scale-pop-70);
			}

			&:disabled {
				cursor: not-allowed;
				opacity: 0.5;
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
		font-weight: 500;
		font-size: 14px;
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
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}

	.ssh-key-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.ssh-key-name {
		color: var(--clr-scale-ntrl-0);
		font-weight: 500;
		font-size: 14px;
	}

	.ssh-key-fingerprint {
		color: var(--clr-scale-ntrl-30);
		font-size: 13px;
		font-family: var(--fontfamily-mono);
	}

	.delete-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		background-color: transparent;
		color: var(--clr-scale-ntrl-30);
		font-size: 18px;
		line-height: 1;
		cursor: pointer;
		transition: all var(--transition-medium);

		&:hover {
			border-color: var(--clr-scale-ntrl-30);
			background-color: var(--clr-scale-ntrl-10);
			color: var(--clr-scale-ntrl-50);
		}
	}

	.add-key-button {
		display: flex;
		align-items: center;
		margin-bottom: 8px;
		padding: 12px;
		gap: 8px;
		border: 1px dashed var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: transparent;
		color: var(--clr-scale-ntrl-50);
		font-size: 14px;
		cursor: pointer;
		transition: all var(--transition-medium);

		&:hover:not(:disabled) {
			border-color: var(--clr-scale-pop-70);
			color: var(--clr-scale-pop-70);
		}

		&:disabled {
			cursor: not-allowed;
			opacity: 0.5;
		}
	}

	.add-key-icon {
		font-size: 18px;
		line-height: 1;
	}

	.loading,
	.no-keys {
		padding: 24px;
		color: var(--clr-scale-ntrl-30);
		font-size: 14px;
		text-align: center;
	}

	.additional-info {
		display: flex;
		flex-direction: column;
		width: 100%;
	}

	.additional-info__fields {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 12px;
	}

	.save-button {
		align-self: flex-end;
		margin-top: 16px;
		padding: 8px 16px;
		border: none;
		border-radius: var(--radius-m);
		background-color: var(--clr-scale-pop-70);
		color: var(--clr-core-ntrl-100);
		font-weight: 500;
		font-size: 14px;
		cursor: pointer;
		transition: background-color var(--transition-medium);

		&:hover:not(:disabled) {
			background-color: var(--clr-scale-pop-60);
		}

		&:disabled {
			cursor: not-allowed;
			opacity: 0.7;
		}
	}

	.ssh-token-modal-backdrop {
		display: flex;
		z-index: 1000;
		position: fixed;
		top: 0;
		right: 0;
		bottom: 0;
		left: 0;
		align-items: center;
		justify-content: center;
		background-color: rgba(0, 0, 0, 0.5);
	}

	.ssh-token-modal {
		display: flex;
		flex-direction: column;
		width: 600px;
		max-width: 90vw;
		padding: 24px;
		gap: 16px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}

	.ssh-token-modal__title {
		color: var(--clr-scale-ntrl-0);
		font-weight: 600;
		font-size: 18px;
	}

	.ssh-token-modal__description {
		color: var(--clr-scale-ntrl-30);
		font-size: 14px;
		line-height: 1.5;
	}

	.ssh-token-modal__code {
		position: relative;
		padding: 16px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		color: var(--clr-scale-ntrl-0);
		font-size: 13px;
		line-height: 1.4;
		font-family: var(--fontfamily-mono);
		word-break: break-all;
	}

	.ssh-token-modal__copy-button {
		position: absolute;
		top: 8px;
		right: 8px;
		padding: 4px 8px;
		border: none;
		border-radius: var(--radius-s);
		background-color: var(--clr-scale-ntrl-70);
		color: var(--clr-core-ntrl-100);
		font-size: 12px;
		cursor: pointer;
		transition: background-color var(--transition-medium);

		&:hover {
			background-color: var(--clr-scale-ntrl-50);
		}
	}

	.ssh-token-modal__note {
		color: var(--clr-scale-ntrl-30);
		font-size: 13px;
		line-height: 1.5;
	}

	.ssh-token-modal__controls {
		display: flex;
		justify-content: flex-end;
		margin-top: 8px;
	}

	.ssh-token-modal__close-button {
		padding: 8px 16px;
		border: none;
		border-radius: var(--radius-m);
		background-color: var(--clr-scale-pop-70);
		color: var(--clr-core-ntrl-100);
		font-size: 14px;
		cursor: pointer;
		transition: background-color var(--transition-medium);

		&:hover {
			background-color: var(--clr-scale-pop-60);
		}
	}
</style>
