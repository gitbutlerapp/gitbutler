<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import CodegenRowUi from '$components/CodegenRowUi.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { ATTACHMENT_SERVICE } from '$lib/codegen/attachmentService.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import {
		CodegenCommitDropHandler,
		CodegenFileDropHandler,
		CodegenHunkDropHandler
	} from '$lib/codegen/dropzone';
	import { extractLastMessage, getTodos } from '$lib/codegen/messages';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { truncate } from '$lib/utils/string';
	import { inject } from '@gitbutler/core/context';
	import type { ClaudeStatus, PromptAttachment } from '$lib/codegen/types';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
		selected: boolean;
		status: ClaudeStatus;
		onselect?: () => void;
	};

	const { projectId, stackId, branchName, selected, status, onselect }: Props = $props();

	const uiState = inject(UI_STATE);
	const laneState = uiState.lane(stackId);

	const claudeService = inject(CLAUDE_CODE_SERVICE);
	const messages = claudeService.messages({ projectId, stackId });
	const attachmentService = inject(ATTACHMENT_SERVICE);

	function addAttachment(items: PromptAttachment[]) {
		return attachmentService.add(branchName, items);
	}

	const handlers = $derived([
		new CodegenCommitDropHandler(stackId, branchName, addAttachment),
		new CodegenFileDropHandler(stackId, branchName, addAttachment),
		new CodegenHunkDropHandler(stackId, branchName, addAttachment)
	]);

	function handleSelection() {
		laneState.selection.set(
			selected ? undefined : { branchName, codegen: true, previewOpen: true }
		);
		onselect?.();
	}
</script>

<Dropzone {handlers}>
	{#snippet overlay({ hovered, activated })}
		<CardOverlay {hovered} {activated} label="Reference" />
	{/snippet}

	<ReduxResult {projectId} result={messages.result}>
		{#snippet children(messages)}
			{@const lastMessage = extractLastMessage(messages)}
			{@const text = lastMessage ? truncate(lastMessage, 360, 8) : undefined}
			{@const todoData = getTodos(messages)}
			{@const todos = {
				completed: todoData.filter((t) => t.status === 'completed').length,
				total: todoData.length
			}}
			<CodegenRowUi
				{branchName}
				{status}
				{selected}
				{text}
				{handlers}
				{todos}
				onselect={handleSelection}
			/>
		{/snippet}
	</ReduxResult>
</Dropzone>

<style lang="postcss">
</style>
