import type { LoadableData } from '$lib/network/types';

export type ApiNotificationSettings = {
	receive_chat_mention_emails: boolean;
	receive_issue_resolution_emails: boolean;
	receive_issue_creation_emails: boolean;
	receive_review_branch_emails: boolean;
	receive_sign_off_emails: boolean;
};

export type NotificationSettings = {
	receiveChatMentionEmails: boolean;
	receiveIssueResolutionEmails: boolean;
	receiveIssueCreationEmails: boolean;
	receiveReviewBranchEmails: boolean;
	receiveSignOffEmails: boolean;
};

export function apiToNotificationSettings(
	apiSettings: ApiNotificationSettings
): NotificationSettings {
	return {
		receiveChatMentionEmails: apiSettings.receive_chat_mention_emails,
		receiveIssueResolutionEmails: apiSettings.receive_issue_resolution_emails,
		receiveIssueCreationEmails: apiSettings.receive_issue_creation_emails,
		receiveReviewBranchEmails: apiSettings.receive_review_branch_emails,
		receiveSignOffEmails: apiSettings.receive_sign_off_emails
	};
}

export const NOTIFICATION_SETTINGS_KEY = 'notification-settings';

export type LoadableNotificationSettings = LoadableData<
	NotificationSettings,
	typeof NOTIFICATION_SETTINGS_KEY
>;

export type PatchNotificationSettingsParams = {
	receiveChatMentionEmails?: boolean;
	receiveIssueResolutionEmails?: boolean;
	receiveIssueCreationEmails?: boolean;
	receiveReviewBranchEmails?: boolean;
	receiveSignOffEmails?: boolean;
};

export type ApiPatchNotificationSettingsParams = {
	receive_chat_mention_emails?: boolean;
	receive_issue_resolution_emails?: boolean;
	receive_issue_creation_emails?: boolean;
	receive_review_branch_emails?: boolean;
	receive_sign_off_emails?: boolean;
};

export function notificationSettingsToApi(
	settings: PatchNotificationSettingsParams
): ApiPatchNotificationSettingsParams {
	return {
		receive_chat_mention_emails: settings.receiveChatMentionEmails,
		receive_issue_resolution_emails: settings.receiveIssueResolutionEmails,
		receive_issue_creation_emails: settings.receiveIssueCreationEmails,
		receive_review_branch_emails: settings.receiveReviewBranchEmails,
		receive_sign_off_emails: settings.receiveSignOffEmails
	};
}
