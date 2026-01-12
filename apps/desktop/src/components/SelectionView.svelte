<script lang="ts">
	import FilePreviewPlaceholder from '$components/FilePreviewPlaceholder.svelte';
	import FileViewHeaderWrapper from '$components/FileViewHeaderWrapper.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import UnifiedDiffView from '$components/UnifiedDiffView.svelte';
	import { BUTBOT_SERVICE, type ForgeReviewFilter } from '$lib/ai/butbot.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { isExecutableStatus } from '$lib/hunks/change';
	import { DIFF_SERVICE } from '$lib/hunks/diffService.svelte';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { readKey, type SelectionId } from '$lib/selection/key';
	import { inject } from '@gitbutler/core/context';
	import { Button, Icon, Markdown, ScrollableContainer } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		selectionId?: SelectionId;
		draggableFiles?: boolean;
		diffOnly?: boolean;
		onclose?: () => void;
		testId?: string;
		scrollContainer?: HTMLDivElement;
		bottomBorder?: boolean;
		targetBranch?: string;
	};

	let {
		projectId,
		selectionId,
		draggableFiles: draggable,
		diffOnly,
		onclose,
		testId,
		scrollContainer,
		bottomBorder,
		targetBranch
	}: Props = $props();

	const idSelection = inject(FILE_SELECTION_MANAGER);
	const diffService = inject(DIFF_SERVICE);

	const selection = $derived(selectionId ? idSelection.valuesReactive(selectionId) : undefined);
	const lastAdded = $derived(selectionId ? idSelection.getById(selectionId).lastAdded : undefined);

	const selectedFile = $derived.by(() => {
		if (!selectionId || !selection) return;
		if (selection.current.length === 0) return;
		if (selection.current.length === 1 || !$lastAdded) return selection.current[0];
		return readKey($lastAdded.key);
	});

	const stackId = $derived(
		selectionId && `stackId` in selectionId ? selectionId.stackId : undefined
	);

	const selectable = $derived(selectionId?.type === 'worktree');

	const messageId = $derived(`branch-chat-${projectId}-${targetBranch}`);

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));
	const butbotService = inject(BUTBOT_SERVICE);
	const [chatWithBranch, result] = butbotService.forgeBranchChat;

	let message = $state('');
	let answerStream = $state('');

	$effect(() => {
		butbotService.listenForTokens(projectId, messageId, (token: string) => {
			answerStream += token;
		});
	});

	async function handleKeydown(event: KeyboardEvent) {
		if (!targetBranch) return;
		if (event.key === 'Enter' && message.trim() !== '') {
			answerStream = '';
			const content = message.trim();
			message = '';
			await chatWithBranch({
				branch: targetBranch,
				filter: 'all',
				projectId,
				chatMessages: [{ type: 'user', content }],
				messageId,
				model: 'gpt-4.1'
			});
		}
	}

	async function summarizePeriod(period: 'today' | 'week' | 'month') {
		if (!targetBranch) return;
		answerStream = '';
		let prompt = '';
		let filter: ForgeReviewFilter = 'all';

		switch (period) {
			case 'today':
				filter = 'today';
				prompt = 'Summarize the changes made to this branch today. Be concise.';
				break;
			case 'week':
				filter = 'thisWeek';
				prompt = 'Summarize the changes made to this branch this week. Be concise.';
				break;
			case 'month':
				filter = 'thisMonth';
				prompt = 'Summarize the changes made to this branch this month. Be concise.';
				break;
		}

		await chatWithBranch({
			branch: targetBranch,
			filter,
			projectId,
			chatMessages: [{ type: 'user', content: prompt }],
			messageId,
			model: 'gpt-4.1'
		});
	}

	function handleClear() {
		message = '';
		answerStream = '';
	}
</script>

<div class="selection-view" data-testid={testId}>
	{#if selectedFile}
		{@const changeQuery = idSelection.changeByKey(projectId, selectedFile)}
		<ReduxResult {projectId} result={changeQuery.result}>
			{#snippet children(change)}
				{@const diffQuery = diffService.getDiff(projectId, change)}
				{@const isExecutable = isExecutableStatus(change.status)}
				<ReduxResult {projectId} result={diffQuery.result}>
					{#snippet children(diff, env)}
						<div
							class="selected-change-item"
							class:bottom-border={bottomBorder}
							data-remove-from-panning
						>
							{#if !diffOnly}
								<FileViewHeaderWrapper
									selectionId={selectedFile}
									projectId={env.projectId}
									{scrollContainer}
									{change}
									{diff}
									{draggable}
									executable={isExecutable}
									onCloseClick={onclose}
								/>
							{/if}
							<UnifiedDiffView
								projectId={env.projectId}
								{stackId}
								commitId={selectedFile.type === 'commit' ? selectedFile.commitId : undefined}
								{draggable}
								{change}
								{diff}
								{selectable}
								selectionId={selectedFile}
								topPadding={diffOnly}
							/>
						</div>
					{/snippet}
				</ReduxResult>
			{/snippet}
		</ReduxResult>
	{:else}
		{#if !answerStream}
			<FilePreviewPlaceholder />
		{/if}
		{#if targetBranch && $aiGenEnabled}
			<div class="select-some__input-wrapper">
				<span class="text-13 select-some__caption">Summarize changes in this branch</span>
				<div>
					<Button
						kind="outline"
						disabled={result.current.isLoading}
						onclick={() => summarizePeriod('today')}>today</Button
					>
					<Button
						kind="outline"
						disabled={result.current.isLoading}
						onclick={() => summarizePeriod('week')}>this week</Button
					>
					<Button
						kind="outline"
						disabled={result.current.isLoading}
						onclick={() => summarizePeriod('month')}>this month</Button
					>
				</div>

				<span class="text-13 select-some__caption">or ask a question</span>
				<input
					bind:value={message}
					class="search-input text-13"
					type="text"
					placeholder="Ask about this branch..."
					onkeydown={handleKeydown}
					disabled={result.current.isLoading}
				/>
				{#if result.current.isSuccess && answerStream}
					<div class="select-some__answer-wrapper">
						<ScrollableContainer>
							<div class="text-13 select-some__answer">
								<Markdown content={result.current.data} />
							</div>
						</ScrollableContainer>
					</div>

					<Button onclick={handleClear} kind="outline">Clear</Button>
				{:else if result.current.isLoading && answerStream}
					<div class="select-some__answer-wrapper">
						<ScrollableContainer wide>
							<div class="text-13 select-some__answer">
								<Markdown content={answerStream} />
							</div>
						</ScrollableContainer>
					</div>
				{:else if result.current.isLoading}
					<Icon name="spinner" />
				{/if}
			</div>
		{/if}
	{/if}
</div>

<style lang="postcss">
	.selection-view {
		display: flex;
		flex-grow: 1;
		width: 100%;
		height: 100%;
	}
	.selected-change-item {
		width: 100%;
		background-color: var(--clr-bg-1);

		&.bottom-border {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.select-some__caption {
		margin-top: 28px;
		color: var(--clr-text-2);
		opacity: 0.6;
	}

	.select-some__input-wrapper {
		display: flex;
		flex-direction: column;
		align-items: center;
		width: 100%;
		height: 100%;
		padding: 8px;
		gap: 8px;
	}

	.select-some__answer {
		position: relative;
		width: 100%;
		height: 100%;

		margin-top: 16px;
		padding: 12px;
	}

	.search-input {
		width: 100%;
		max-width: 460px;
		height: 100%;
		padding-left: 8px;
		padding: 6px 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s) var(--radius-m) var(--radius-m) var(--radius-s);
		background-color: var(--clr-bg-1);
		transition: opacity 0.1s;

		&:focus-within {
			outline: none;
		}

		&:hover,
		&:focus {
			border-color: var(--clr-border-1);
		}

		&::placeholder {
			color: var(--clr-text-3);
		}
	}
</style>
