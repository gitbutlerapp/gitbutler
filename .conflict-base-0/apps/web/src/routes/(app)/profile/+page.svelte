<script lang="ts">
	import { AuthService } from '$lib/auth/authService.svelte';
	import { featureShowOrganizations, featureShowProjectPage } from '$lib/featureFlags';
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
</script>

<svelte:head>
	<title>GitButler | User</title>
</svelte:head>

{#if !$token}
	<p>Unauthorized</p>
{:else if !$user?.id}
	<p>Loading...</p>
{:else}
	<div class="profile">
		<h1>Your Profile</h1>
		<div><b>Login</b>: {$user?.login}</div>
		<div><b>Email</b>: {$user?.email}</div>
		<div><b>Joined</b>: {$user?.created_at}</div>
		<div><b>Supporter</b>: {$user?.supporter}</div>
	</div>
{/if}

<div class="settings-section">
	<h1>Notification settings</h1>

	<Loading loadable={notificationSettings.current}>
		{#snippet children(notificationSettings)}
			<SectionCard labelFor="receive-chat-mention-emails" orientation="row">
				{#snippet title()}Receive chat message mention emails{/snippet}
				{#snippet caption()}
					Receive emails everytime you are mentioned in a message.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="receive-chat-mention-emails"
						checked={notificationSettings.receiveChatMentionEmails}
						disabled={updatingReceiveChatMentionEmails}
						onclick={() =>
							updateReceiveChatMentionEmails(!notificationSettings.receiveChatMentionEmails)}
					/>
				{/snippet}
			</SectionCard>

			<SectionCard labelFor="receive-issue-creation-emails" orientation="row">
				{#snippet title()}Receive issue creation emails{/snippet}
				{#snippet caption()}
					Receive emails for every new issue created in changes you are involved in.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="receive-issue-creation-emails"
						checked={notificationSettings.receiveIssueCreationEmails}
						disabled={updatingReceiveIssueCreationEmails}
						onclick={() =>
							updateReceiveIssueCreationEmails(!notificationSettings.receiveIssueCreationEmails)}
					/>
				{/snippet}
			</SectionCard>

			<SectionCard labelFor="receive-issue-resolution-emails" orientation="row">
				{#snippet title()}Receive issue status emails{/snippet}
				{#snippet caption()}
					Receive emails for every status update of issues in changes you are involved in.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="receive-issue-resolution-emails"
						checked={notificationSettings.receiveIssueResolutionEmails}
						disabled={updatingReceiveIssueResolutionEmails}
						onclick={() =>
							updateReceiveIssueResolutionEmails(
								!notificationSettings.receiveIssueResolutionEmails
							)}
					/>
				{/snippet}
			</SectionCard>

			<SectionCard labelFor="receive-review-branch-emails" orientation="row">
				{#snippet title()}Receive branch version update emails{/snippet}
				{#snippet caption()}
					Receive emails for every time a new review branch version is created.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="receive-review-branch-emails"
						checked={notificationSettings.receiveReviewBranchEmails}
						disabled={updatingReceiveReviewBranchEmails}
						onclick={() =>
							updateReceiveReviewBranchEmails(!notificationSettings.receiveReviewBranchEmails)}
					/>
				{/snippet}
			</SectionCard>

			<SectionCard labelFor="receive-sign-off-emails" orientation="row">
				{#snippet title()}Receive change status update emails{/snippet}
				{#snippet caption()}
					Receive emails for every update on the review status of changes your involved in.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="receive-sign-off-emails"
						checked={notificationSettings.receiveSignOffEmails}
						disabled={updatingReceiveSignOffEmails}
						onclick={() => updateReceiveSignOffEmails(!notificationSettings.receiveSignOffEmails)}
					/>
				{/snippet}
			</SectionCard>
		{/snippet}
	</Loading>
</div>

<div class="settings-section">
	<h1>Experimental settings</h1>
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
</div>

<style>
	h1 {
		font-size: 1.5rem;
		margin-bottom: 10px;
	}
	.profile,
	.settings-section {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 32px;
	}
</style>
