import { usageStats, formatMessages, type Message } from '$lib/codegen/messages';
import { SettingsService } from '$lib/config/appSettingsV2';
import { InjectionToken } from '@gitbutler/core/context';
import { get } from 'svelte/store';
import type { ClaudeCodeService } from '$lib/codegen/claude';
import type { ThinkingLevel, ModelType } from '$lib/codegen/types';
import type { EventProperties } from '$lib/state/customHooks.svelte';

export const CODEGEN_ANALYTICS = new InjectionToken<CodegenAnalytics>('CodegenAnalytics');

export class CodegenAnalytics {
	constructor(
		private claudeCodeService: ClaudeCodeService,
		private settingsService: SettingsService
	) {}

	async getCodegenProperties(args: {
		projectId: string;
		stackId: string;
		message: string;
		thinkingLevel: ThinkingLevel;
		model: ModelType;
	}): Promise<EventProperties> {
		try {
			// Fetch current messages and stack active status
			const [messages, isStackActiveResult] = await Promise.all([
				this.claudeCodeService.fetchMessages({
					projectId: args.projectId,
					stackId: args.stackId
				}),
				this.claudeCodeService.fetchIsStackActive({
					projectId: args.projectId,
					stackId: args.stackId
				})
			]);

			// Get settings for Claude preferences
			const settings = get(this.settingsService.appSettings);

			// Calculate usage stats
			const usage = usageStats(messages);

			// Format messages to get accurate user message count
			// Using empty permission requests array as fallback since we don't have a fetch method for it yet
			const formattedMessages = formatMessages(messages, [], isStackActiveResult);
			const totalMessagesSent = this.countUserMessages(formattedMessages);

			const claudeMetrics = {
				thinkingLevel: args.thinkingLevel,
				model: args.model,
				notifyOnCompletion: settings?.claude?.notifyOnCompletion ?? false,
				notifyOnPermissionRequest: settings?.claude?.notifyOnPermissionRequest ?? false,
				autoCommitAfterCompletion: settings?.claude?.autoCommitAfterCompletion ?? true,
				dangerouslySkipPermissions: settings?.claude?.dangerouslyAllowAllPermissions ?? false,
				tokensUsed: usage.tokens,
				totalMessagesSent: totalMessagesSent
			};

			return namespaceProps(claudeMetrics, 'claude');
		} catch (_e) {
			return namespaceProps(
				{
					thinkingLevel: args.thinkingLevel,
					model: args.model
				},
				'claude'
			);
		}
	}

	private countUserMessages(formattedMessages: Message[]): number {
		return formattedMessages.filter((message) => message.type === 'user').length;
	}
}

function namespaceProps(props: EventProperties, namespace: string): EventProperties {
	const namespacedProps: EventProperties = {};
	for (const [key, value] of Object.entries(props)) {
		namespacedProps[`${namespace}:${key}`] = value;
	}
	return namespacedProps;
}
