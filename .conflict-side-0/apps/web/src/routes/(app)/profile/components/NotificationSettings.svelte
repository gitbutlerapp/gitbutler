<script lang="ts">
	import { SectionCard, Toggle, Spacer } from '@gitbutler/ui';
	import type { NotificationSettingsService } from '@gitbutler/shared/settings/notificationSettingsService';
	import type { NotificationSettings as NotificationSettingsType } from '@gitbutler/shared/settings/types';

	interface Props {
		notificationSettings: NotificationSettingsType;
		notificationSettingsService: NotificationSettingsService;
	}

	let { notificationSettings, notificationSettingsService }: Props = $props();

	let updatingReceiveChatMentionEmails = $state(false);
	let updatingReceiveChatReplyEmails = $state(false);
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
		await notificationSettingsService.updateNotificationSettings({
			receiveSignOffEmails: value
		});
		updatingReceiveSignOffEmails = false;
	}
</script>

<Spacer />

<div class="stack-v gap-8">
	<h2 class="text-15 text-bold">Notification settings</h2>
	<p class="text-12 text-body clr-text-2">
		Manage your email notification preferences for various activities within GitButler.
	</p>
</div>

<div class="stack-v">
	<SectionCard orientation="row" roundedBottom={false} labelFor="receive-chat-mention-emails">
		{#snippet title()}
			Chat message mention emails
		{/snippet}
		{#snippet caption()}
			Emails when you are mentioned in a message.
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

	<SectionCard
		orientation="row"
		roundedBottom={false}
		roundedTop={false}
		labelFor="receive-chat-reply-emails"
	>
		{#snippet title()}
			Chat message reply emails
		{/snippet}
		{#snippet caption()}
			Emails when you receive a reply to a chat message.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="receive-chat-reply-emails"
				checked={notificationSettings.receiveChatReplyEmails}
				disabled={updatingReceiveChatReplyEmails}
				onclick={() => updateReceiveChatReplyEmails(!notificationSettings.receiveChatReplyEmails)}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard
		orientation="row"
		roundedBottom={false}
		roundedTop={false}
		labelFor="receive-issue-creation-emails"
	>
		{#snippet title()}
			Issue creation emails
		{/snippet}
		{#snippet caption()}
			Emails for new issues created in changes you are involved in.
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

	<SectionCard
		orientation="row"
		roundedBottom={false}
		roundedTop={false}
		labelFor="receive-issue-resolution-emails"
	>
		{#snippet title()}
			Issue status emails
		{/snippet}
		{#snippet caption()}
			Emails for status updates of issues in changes you are involved in.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="receive-issue-resolution-emails"
				checked={notificationSettings.receiveIssueResolutionEmails}
				disabled={updatingReceiveIssueResolutionEmails}
				onclick={() =>
					updateReceiveIssueResolutionEmails(!notificationSettings.receiveIssueResolutionEmails)}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard
		orientation="row"
		roundedBottom={false}
		roundedTop={false}
		labelFor="receive-review-branch-emails"
	>
		{#snippet title()}
			Branch version update emails
		{/snippet}
		{#snippet caption()}
			Emails when a new review branch version is created.
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

	<SectionCard orientation="row" roundedTop={false} labelFor="receive-sign-off-emails">
		{#snippet title()}
			Change status update emails
		{/snippet}
		{#snippet caption()}
			Emails for updates on the review status of changes you are involved in.
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
</div>

<style lang="postcss">
	.notification-settings {
		display: flex;
		flex-direction: column;
	}
</style>
