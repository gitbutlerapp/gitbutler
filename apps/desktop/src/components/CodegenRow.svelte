<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { ATTACHMENT_SERVICE } from '$lib/codegen/attachmentService.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import {
		CodegenCommitDropHandler,
		CodegenFileDropHandler,
		CodegenHunkDropHandler
	} from '$lib/codegen/dropzone';
	import { extractLastMessage, usageStats, lastInteractionTime } from '$lib/codegen/messages';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { formatNumber } from '$lib/utils/number';
	import { truncate } from '$lib/utils/string';
	import { inject } from '@gitbutler/core/context';
	import { Icon, TimeAgo, Tooltip } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { slide, fade } from 'svelte/transition';
	import type { ClaudeStatus, PromptAttachment } from '$lib/codegen/types';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
		selected: boolean;
		status: ClaudeStatus;
	};

	const { projectId, stackId, branchName, selected, status }: Props = $props();

	const uiState = inject(UI_STATE);
	const laneState = uiState.lane(stackId);

	const claudeService = inject(CLAUDE_CODE_SERVICE);
	const messages = claudeService.messages({ projectId, stackId });

	let active = $state(false);

	function getCurrentIconName(): keyof typeof iconsJson {
		if (status === 'running' || status === 'compacting') {
			return 'spinner';
		}
		return 'ai';
	}

	const attachmentService = inject(ATTACHMENT_SERVICE);

	function addAttachment(items: PromptAttachment[]) {
		return attachmentService.add(branchName, items);
	}

	const handlers = $derived([
		new CodegenCommitDropHandler(stackId, branchName, addAttachment),
		new CodegenFileDropHandler(stackId, branchName, addAttachment),
		new CodegenHunkDropHandler(stackId, branchName, addAttachment)
	]);
</script>

<Dropzone {handlers}>
	{#snippet overlay({ hovered, activated })}
		<CardOverlay {hovered} {activated} label="Reference" />
	{/snippet}
	<button
		type="button"
		class="codegen-row"
		class:selected
		class:active
		onclick={() => {
			laneState.selection.set({ branchName, codegen: true, previewOpen: true });
		}}
		use:focusable={{
			onAction: () => {
				laneState.selection.set({ branchName, codegen: true, previewOpen: true });
			},
			onActive: (value) => (active = value),
			focusable: true
		}}
	>
		{#if selected}
			<div
				class="indicator"
				class:selected
				class:active
				in:slide={{ axis: 'x', duration: 150 }}
			></div>
		{/if}

		<ReduxResult {projectId} result={messages.result}>
			{#snippet children(messages)}
				{@const lastMessage = extractLastMessage(messages)}
				{@const lastSummary = lastMessage ? truncate(lastMessage, 360, 8) : undefined}

				<Icon name={getCurrentIconName()} color="var(--clr-theme-purp-element)" />
				<h3 class="text-13 text-semibold truncate codegen-row__title">{lastSummary}</h3>
			{/snippet}
		</ReduxResult>
	</button>
</Dropzone>

<style lang="postcss">
	.codegen-row {
		display: flex;
		position: relative;
		width: 100%;
		padding: 12px;
		padding-left: 14px;
		gap: 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-theme-purp-bg);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-theme-purp-bg-muted);
		}

		/* Selected in focus */
		&.active.selected {
			background-color: var(--clr-theme-purp-bg-muted);
		}
	}

	.codegen-row__title {
		color: var(--clr-theme-purp-on-soft);
	}

	.indicator {
		position: absolute;
		top: 50%;
		left: 0;
		width: 4px;
		height: 45%;
		transform: translateY(-50%);
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		background-color: var(--clr-theme-purp-element);
		transition: transform var(--transition-fast);

		&.active {
			background-color: var(--clr-theme-purp-element);
		}
	}
</style>
