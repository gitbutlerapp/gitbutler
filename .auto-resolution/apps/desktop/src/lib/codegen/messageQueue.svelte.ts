import { CLAUDE_CODE_SERVICE, ClaudeCodeService } from '$lib/codegen/claude';
import {
	messageQueueSelectors,
	messageQueueSlice,
	type MessageQueue
} from '$lib/codegen/messageQueueSlice';
import { currentStatus, isCompletedWithStatus } from '$lib/codegen/messages';
import { CODEGEN_ANALYTICS, CodegenAnalytics } from '$lib/soup/codegenAnalytics';
import { CLIENT_STATE, type ClientState } from '$lib/state/clientState.svelte';
import {
	UI_STATE,
	type GlobalStore,
	type StackState,
	type UiState
} from '$lib/state/uiState.svelte';
import { inject } from '@gitbutler/core/context';
import { chipToasts } from '@gitbutler/ui';
import type {
	ModelType,
	PermissionMode,
	PromptAttachment,
	ThinkingLevel
} from '$lib/codegen/types';
import type { Reactive } from '@gitbutler/shared/storeUtils';

/**
 * Performs the actual message sending logic.
 * Shared by both MessageSender instances and MessageQueueProcessor.
 */
async function performSend({
	prompt,
	projectId,
	stackId,
	thinkingLevel,
	model,
	permissionMode,
	laneState,
	claudeCodeService,
	codegenAnalytics,
	attachments
}: {
	prompt: string;
	projectId: string;
	stackId: string;
	thinkingLevel: ThinkingLevel;
	model: ModelType;
	permissionMode: PermissionMode;
	laneState: GlobalStore<StackState> | undefined;
	claudeCodeService: ClaudeCodeService;
	codegenAnalytics: CodegenAnalytics;
	attachments?: PromptAttachment[];
}) {
	if (prompt.startsWith('/compact')) {
		await claudeCodeService.compactHistory({
			projectId,
			stackId
		});
		return;
	}

	// Handle /add-dir command
	if (prompt.startsWith('/add-dir ')) {
		const path = prompt.slice('/add-dir '.length).trim();
		if (path) {
			const isValid = await claudeCodeService.verifyPath({ projectId, path });
			if (isValid) {
				laneState?.addedDirs.add(path);
			} else {
				chipToasts.error(`Invalid directory path: ${path}`);
			}
		}
		return;
	}

	if (prompt.startsWith('/')) {
		chipToasts.warning('Slash commands are not yet supported');
		return;
	}

	// Await analytics data before sending message
	const analyticsProperties = await codegenAnalytics.getCodegenProperties({
		projectId,
		stackId,
		message: prompt,
		thinkingLevel,
		model
	});

	claudeCodeService.sendMessage[0](
		{
			projectId,
			stackId,
			message: prompt,
			thinkingLevel,
			model,
			permissionMode,
			disabledMcpServers: laneState?.disabledMcpServers.current ?? [],
			addDirs: laneState?.addedDirs.current ?? [],
			attachments
		},
		{ properties: analyticsProperties }
	);
}

export class MessageQueueProcessor {
	private clientState: ClientState;
	private claudeCodeService: ClaudeCodeService;
	private codegenAnalytics: CodegenAnalytics;
	private uiState: UiState;

	constructor() {
		this.clientState = inject(CLIENT_STATE);
		this.claudeCodeService = inject(CLAUDE_CODE_SERVICE);
		this.codegenAnalytics = inject(CODEGEN_ANALYTICS);
		this.uiState = inject(UI_STATE);

		const queueIds = $derived(messageQueueSelectors.selectIds(this.clientState.messageQueue));

		// By looping over the IDs first rather than doing the full array, we avoid
		// extra recomputations when one of the message queues changes.
		$effect(() => {
			for (const id of queueIds) {
				const queue = $derived(messageQueueSelectors.selectById(this.clientState.messageQueue, id));
				if (queue) {
					$effect(() => {
						this.handleQueue(queue);
					});
				}
			}
		});
	}

	private handleQueue(queue: MessageQueue) {
		$effect(() => {
			if (queue.messages.length === 0 && queue.isProcessing) {
				this.clientState.dispatch(
					messageQueueSlice.actions.upsert({
						...queue,
						isProcessing: false
					})
				);
			}
		});

		const isActive = this.claudeCodeService.isStackActive(queue.projectId, queue.stackId);
		const events = this.claudeCodeService.messages({
			projectId: queue.projectId,
			stackId: queue.stackId
		});

		$effect(() => {
			if (
				queue.messages.length > 0 &&
				!queue.isProcessing &&
				events.response &&
				isActive.response !== undefined &&
				!isActive.response
			) {
				const status = isCompletedWithStatus(events.response, isActive.response ?? false);
				const laneState = this.uiState.lane(queue.stackId);

				if (
					(status.type === 'completed' && status.code === 0) ||
					status.type === 'noMessagesSent'
				) {
					const message = queue.messages[0]!;
					this.clientState.dispatch(
						messageQueueSlice.actions.upsert({
							...queue,
							messages: queue.messages.slice(1),
							isProcessing: true
						})
					);

					performSend({
						prompt: message.prompt,
						projectId: queue.projectId,
						stackId: queue.stackId,
						thinkingLevel: message.thinkingLevel,
						model: message.model,
						permissionMode: message.permissionMode,
						laneState,
						claudeCodeService: this.claudeCodeService,
						codegenAnalytics: this.codegenAnalytics,
						attachments: message.attachments
					}).finally(() => {
						const queue2 = messageQueueSelectors.selectById(
							this.clientState.messageQueue,
							queue.stackId
						);
						if (!queue2) return;
						this.clientState.dispatch(
							messageQueueSlice.actions.upsert({
								...queue2,
								isProcessing: false
							})
						);
					});
				}
			}
		});
	}
}

export class MessageSender {
	private projectId: Reactive<string>;
	private selectedBranch: Reactive<{ stackId: string; head: string } | undefined>;
	private thinkingLevel: Reactive<ThinkingLevel>;
	private model: Reactive<ModelType>;
	private permissionMode: Reactive<PermissionMode>;
	private uiState: UiState;
	private claudeCodeService: ClaudeCodeService;
	private codegenAnalytics: CodegenAnalytics;
	private clientState: ClientState;

	constructor({
		projectId,
		selectedBranch,
		thinkingLevel,
		model,
		permissionMode
	}: {
		projectId: Reactive<string>;
		selectedBranch: Reactive<{ stackId: string; head: string } | undefined>;
		thinkingLevel: Reactive<ThinkingLevel>;
		model: Reactive<ModelType>;
		permissionMode: Reactive<PermissionMode>;
	}) {
		this.projectId = projectId;
		this.selectedBranch = selectedBranch;
		this.thinkingLevel = thinkingLevel;
		this.model = model;
		this.permissionMode = permissionMode;
		this.uiState = inject(UI_STATE);
		this.claudeCodeService = inject(CLAUDE_CODE_SERVICE);
		this.codegenAnalytics = inject(CODEGEN_ANALYTICS);
		this.clientState = inject(CLIENT_STATE);
	}

	private get queue() {
		return messageQueueSelectors
			.selectAll(this.clientState.messageQueue)
			.find(
				(q) =>
					q.head === this.selectedBranch.current?.head &&
					q.stackId === this.selectedBranch.current?.stackId &&
					q.projectId === this.projectId.current
			);
	}

	private get laneState() {
		return this.selectedBranch.current?.stackId
			? this.uiState.lane(this.selectedBranch.current.stackId)
			: undefined;
	}

	get prompt() {
		return this.selectedBranch.current ? (this.laneState?.prompt.current ?? '') : '';
	}

	setPrompt(prompt: string) {
		this.laneState?.prompt.set(prompt);
	}

	async sendMessage(prompt: string, attachments?: PromptAttachment[]) {
		if (!this.selectedBranch.current || !this.laneState || !prompt) return;

		const isActive = await this.claudeCodeService.fetchIsStackActive({
			projectId: this.projectId.current,
			stackId: this.selectedBranch.current.stackId
		});
		const events = await this.claudeCodeService.fetchMessages({
			projectId: this.projectId.current,
			stackId: this.selectedBranch.current.stackId
		});

		const status = currentStatus(events, isActive);
		const canSendImmediately =
			(status === 'disabled' || status === 'enabled') &&
			!this.queue?.isProcessing &&
			(this.queue?.messages.length || 0) === 0;

		if (canSendImmediately) {
			await performSend({
				prompt,
				projectId: this.projectId.current,
				stackId: this.selectedBranch.current.stackId,
				thinkingLevel: this.thinkingLevel.current,
				model: this.model.current,
				permissionMode: this.permissionMode.current,
				laneState: this.laneState,
				claudeCodeService: this.claudeCodeService,
				codegenAnalytics: this.codegenAnalytics,
				attachments
			});
		} else {
			const message = {
				prompt,
				thinkingLevel: this.thinkingLevel.current,
				model: this.model.current,
				permissionMode: this.permissionMode.current,
				attachments
			};

			if (this.queue) {
				this.clientState.dispatch(
					messageQueueSlice.actions.upsert({
						...this.queue,
						messages: [...this.queue.messages, message]
					})
				);
			} else {
				this.clientState.dispatch(
					messageQueueSlice.actions.upsert({
						projectId: this.projectId.current,
						stackId: this.selectedBranch.current.stackId,
						head: this.selectedBranch.current.head,
						isProcessing: false,
						messages: [message]
					})
				);
			}
		}

		this.setPrompt('');
	}
}
