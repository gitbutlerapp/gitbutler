import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
// import { selectAllMessageQueues } from '$lib/codegen/messageQueueSlice';
import { CODEGEN_ANALYTICS } from '$lib/soup/codegenAnalytics';
// import { CLIENT_STATE } from '$lib/state/clientState.svelte';
import { UI_STATE } from '$lib/state/uiState.svelte';
import { inject } from '@gitbutler/core/context';
import { chipToasts } from '@gitbutler/ui';
import type { ModelType, PermissionMode, ThinkingLevel } from '$lib/codegen/types';
import type { Reactive } from '@gitbutler/shared/storeUtils';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';

// export function useMessageQueue() {
// 	const clientState = inject(CLIENT_STATE);
// 	const messageQueue = $derived(selectAllMessageQueues(clientState.messageQueue));

// 	$effect(() => {});
// }

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

	const [sendClaudeMessage] = claudeCodeService.sendMessage;

	const laneState = $derived(
		selectedBranch.current?.stackId ? uiState.lane(selectedBranch.current.stackId) : undefined
	);

	const prompt = $derived(
		selectedBranch.current ? uiState.lane(selectedBranch.current.stackId).prompt.current : ''
	);
	function setPrompt(prompt: string) {
		if (!selectedBranch.current) return;
		uiState.lane(selectedBranch.current.stackId).prompt.set(prompt);
	}
	async function sendMessage() {
		if (!selectedBranch.current) return;
		if (!prompt) return;

		if (prompt.startsWith('/compact')) {
			compactContext();
			return;
		}

		// Handle /add-dir command
		if (prompt.startsWith('/add-dir ')) {
			const path = prompt.slice('/add-dir '.length).trim();
			if (path) {
				const isValid = await claudeCodeService.verifyPath({ projectId: projectId.current, path });
				if (isValid) {
					laneState?.addedDirs.add(path);
					chipToasts.success(`Added directory: ${path}`);
				} else {
					chipToasts.error(`Invalid directory path: ${path}`);
				}
			}
			setPrompt('');
			return;
		}

		if (prompt.startsWith('/')) {
			chipToasts.warning('Slash commands are not yet supported');
			setPrompt('');
			return;
		}

		// Await analytics data before sending message
		const analyticsProperties = await codegenAnalytics.getCodegenProperties({
			projectId: projectId.current,
			stackId: selectedBranch.current.stackId,
			message: prompt,
			thinkingLevel: thinkingLevel.current,
			model: model.current
		});

		const promise = sendClaudeMessage(
			{
				projectId: projectId.current,
				stackId: selectedBranch.current.stackId,
				message: prompt,
				thinkingLevel: thinkingLevel.current,
				model: model.current,
				permissionMode: permissionMode.current,
				disabledMcpServers: uiState.lane(selectedBranch.current.stackId).disabledMcpServers.current,
				addDirs: laneState?.addedDirs.current || []
			},
			{ properties: analyticsProperties }
		);

		setPrompt('');
		await promise;
	}

	async function compactContext() {
		if (!selectedBranch.current) return;

		await claudeCodeService.compactHistory({
			projectId: projectId.current,
			stackId: selectedBranch.current.stackId
		});
	}

	return {
		prompt: reactive(() => prompt),
		setPrompt,
		sendMessage
	};
}
