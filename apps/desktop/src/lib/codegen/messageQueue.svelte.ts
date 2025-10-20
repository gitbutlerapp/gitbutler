import { CLAUDE_CODE_SERVICE, ClaudeCodeService } from '$lib/codegen/claude';
import {
	messageQueueSelectors,
	messageQueueSlice,
	type MessageQueue
} from '$lib/codegen/messageQueueSlice';
import { currentStatus, isCompletedWithStatus } from '$lib/codegen/messages';
import { CODEGEN_ANALYTICS, CodegenAnalytics } from '$lib/soup/codegenAnalytics';
import { CLIENT_STATE } from '$lib/state/clientState.svelte';
import { UI_STATE, type GlobalStore, type StackState } from '$lib/state/uiState.svelte';
import { inject } from '@gitbutler/core/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { chipToasts } from '@gitbutler/ui';
import type { ModelType, PermissionMode, ThinkingLevel, FileAttachment } from '$lib/codegen/types';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export function useMessageQueue() {
	const clientState = inject(CLIENT_STATE);
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const codegenAnalytics = inject(CODEGEN_ANALYTICS);
	const queueIds = $derived(messageQueueSelectors.selectIds(clientState.messageQueue));
	const [sendClaudeMessage] = claudeCodeService.sendMessage;
	const uiState = inject(UI_STATE);

	// By looping over the IDs first rather than doing the full array, we avoid
	// extra recomputations when one of the message queues changes.
	$effect(() => {
		for (const id of queueIds) {
			const queue = $derived(messageQueueSelectors.selectById(clientState.messageQueue, id));
			if (queue) {
				$effect(() => {
					handleQueue(queue);
				});
			}
		}
	});

	function handleQueue(queue: MessageQueue) {
		$effect(() => {
			if (queue.messages.length === 0 && queue.isProcessing) {
				clientState.dispatch(
					messageQueueSlice.actions.upsert({
						...queue,
						isProcessing: false
					})
				);
			}
		});

		const isActive = claudeCodeService.isStackActive(queue.projectId, queue.stackId);
		const events = claudeCodeService.messages({
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
				const laneState = uiState.lane(queue.stackId);

				if (status.type === 'completed' && status.code === 0) {
					const message = queue.messages[0]!;
					clientState.dispatch(
						messageQueueSlice.actions.upsert({
							...queue,
							messages: queue.messages.slice(1),
							isProcessing: true
						})
					);

					sendMessageInner({
						...message,
						projectId: queue.projectId,
						selectedBranch: { head: queue.head, stackId: queue.stackId },
						laneState,
						claudeCodeService,
						codegenAnalytics,
						sendClaudeMessage
					}).finally(() => {
						const queue2 = messageQueueSelectors.selectById(
							clientState.messageQueue,
							queue.stackId
						);
						if (!queue2) return;
						clientState.dispatch(
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

export function useSendMessage({
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
	const uiState = inject(UI_STATE);
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const codegenAnalytics = inject(CODEGEN_ANALYTICS);
	const clientState = inject(CLIENT_STATE);
	const queue = $derived(
		messageQueueSelectors
			.selectAll(clientState.messageQueue)
			.find(
				(q) =>
					q.head === selectedBranch?.current?.head &&
					q.stackId === selectedBranch?.current.stackId &&
					q.projectId === projectId.current
			)
	);

	const [sendClaudeMessage] = claudeCodeService.sendMessage;

	const laneState = $derived(
		selectedBranch.current?.stackId ? uiState.lane(selectedBranch.current.stackId) : undefined
	);
	const prompt = $derived(selectedBranch.current ? (laneState?.prompt.current ?? '') : '');
	function setPrompt(prompt: string) {
		laneState?.prompt.set(prompt);
	}
	async function sendMessage(attachmentsParam?: { id: string; file: File; preview?: string }[]) {
		if (!selectedBranch.current) return;
		if (!laneState) return;
		if (!prompt) return;

		const isActive = await claudeCodeService.fetchIsStackActive({
			projectId: projectId.current,
			stackId: selectedBranch.current.stackId
		});
		const events = await claudeCodeService.fetchMessages({
			projectId: projectId.current,
			stackId: selectedBranch.current.stackId
		});

		const status = currentStatus(events, isActive);

		if (
			(status === 'disabled' || status === 'enabled') &&
			!queue?.isProcessing &&
			(queue?.messages.length || 0) === 0
		) {
			const promise = sendMessageInner({
				prompt,
				projectId: projectId.current,
				laneState,
				selectedBranch: selectedBranch.current,
				thinkingLevel: thinkingLevel.current,
				model: model.current,
				permissionMode: permissionMode.current,
				claudeCodeService,
				codegenAnalytics,
				sendClaudeMessage,
				attachments: attachmentsParam
			});

			setPrompt('');

			await promise;
		} else {
			const message = {
				prompt,
				thinkingLevel: thinkingLevel.current,
				model: model.current,
				permissionMode: permissionMode.current
			};
			if (queue) {
				clientState.dispatch(
					messageQueueSlice.actions.upsert({
						...queue,
						messages: [...queue.messages, message]
					})
				);
			} else {
				clientState.dispatch(
					messageQueueSlice.actions.upsert({
						projectId: projectId.current,
						stackId: selectedBranch.current.stackId,
						head: selectedBranch.current.head,
						isProcessing: false,
						messages: [message]
					})
				);
			}

			setPrompt('');
		}
	}

	return {
		prompt: reactive(() => prompt),
		setPrompt,
		sendMessage
	};
}

async function sendMessageInner({
	prompt,
	projectId,
	laneState,
	selectedBranch,
	thinkingLevel,
	model,
	permissionMode,
	claudeCodeService,
	codegenAnalytics,
	sendClaudeMessage,
	attachments
}: {
	prompt: string;
	projectId: string;
	selectedBranch: { stackId: string; head: string };
	thinkingLevel: ThinkingLevel;
	model: ModelType;
	permissionMode: PermissionMode;
	laneState: GlobalStore<StackState>;
	claudeCodeService: ClaudeCodeService;
	codegenAnalytics: CodegenAnalytics;
	sendClaudeMessage: ClaudeCodeService['sendMessage'][0];
	attachments?: { id: string; file: File; preview?: string }[];
}) {
	if (prompt.startsWith('/compact')) {
		await claudeCodeService.compactHistory({
			projectId,
			stackId: selectedBranch.stackId
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
				chipToasts.success(`Added directory: ${path}`);
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

	// Convert attached files to backend format
	let fileAttachments: FileAttachment[] | undefined = undefined;
	if (attachments && attachments.length > 0) {
		fileAttachments = await Promise.all(
			attachments.map(async (attached: { id: string; file: File; preview?: string }) => {
				// Convert file to base64
				const buffer = await attached.file.arrayBuffer();
				const bytes = new Uint8Array(buffer);
				const binary = bytes.reduce((data, byte) => data + String.fromCharCode(byte), '');
				const base64Content = btoa(binary);

				return {
					id: attached.id,
					name: attached.file.name,
					content: base64Content,
					mimeType: attached.file.type,
					size: attached.file.size
				};
			})
		);
	}

	// Await analytics data before sending message
	const analyticsProperties = await codegenAnalytics.getCodegenProperties({
		projectId,
		stackId: selectedBranch.stackId,
		message: prompt,
		thinkingLevel,
		model
	});

	await sendClaudeMessage(
		{
			projectId,
			stackId: selectedBranch.stackId,
			message: prompt,
			thinkingLevel,
			model,
			permissionMode,
			disabledMcpServers: laneState?.disabledMcpServers.current ?? [],
			addDirs: laneState?.addedDirs.current ?? [],
			attachments: fileAttachments
		},
		{ properties: analyticsProperties }
	);
}
