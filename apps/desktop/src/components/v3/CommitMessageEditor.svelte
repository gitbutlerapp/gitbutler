<script lang="ts">
	import EditorFooter from '$components/v3/editor/EditorFooter.svelte';
	import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
	import MessageEditorInput from '$components/v3/editor/MessageEditorInput.svelte';
	import CommitSuggestions from '$components/v3/editor/commitSuggestions.svelte';
	import DiffInputContext, { type DiffInputContextArgs } from '$lib/ai/diffInputContext.svelte';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { tick } from 'svelte';

	type Props = {
		projectId: string;
		stackId?: string;
		actionLabel: string;
		action: () => void;
		onCancel: () => void;
		disabledAction?: boolean;
		loading?: boolean;
		existingCommitId?: string;

		title: string;
		description: string;

		setTitle: (title: string) => void;
		setDescription: (description: string) => void;
	};

	let {
		projectId,
		stackId,
		actionLabel,
		action,
		onCancel,
		disabledAction,
		loading,
		title,
		description,

		setTitle,
		setDescription,
		existingCommitId
	}: Props = $props();

	const uiState = getContext(UiState);
	const aiService = getContext(AIService);
	const promptService = getContext(PromptService);

	const worktreeService = getContext(WorktreeService);
	const diffService = getContext(DiffService);
	const changeSelection = getContext(ChangeSelectionService);
	const stackService = getContext(StackService);

	const selectedFiles = $derived(changeSelection.list().current);

	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);
	const stackSelection = $derived(stackState?.selection);

	const suggestionsHandler = new CommitSuggestions(aiService, uiState);
	const diffInputArgs = $derived<DiffInputContextArgs>(
		existingCommitId
			? { type: 'commit', projectId, commitId: existingCommitId }
			: { type: 'change-selection', projectId, selectedFiles }
	);
	const diffInputContext = $derived(
		new DiffInputContext(worktreeService, diffService, stackService, diffInputArgs)
	);

	// AI things
	const aiGenEnabled = projectAiGenEnabled(projectId);
	let aiConfigurationValid = $state(false);
	const canUseAI = $derived($aiGenEnabled && aiConfigurationValid);
	let aiIsLoading = $state(false);

	$effect(() => {
		aiService.validateConfiguration().then((valid) => {
			aiConfigurationValid = valid;
		});
	});

	let generatedText = $state<string>('');

	$effect(() => {
		if (generatedText) {
			const { title, description } = splitMessage(generatedText);
			setTitle(title);
			setDescription(description);
			composer?.setText(description);
		}
	});

	function beginGeneration() {
		setTitle('');
		setDescription('');
		generatedText = '';
	}

	async function onAiButtonClick() {
		if (aiIsLoading) return;

		suggestionsHandler.clear();
		aiIsLoading = true;
		await tick();
		try {
			const prompt = promptService.selectedCommitPrompt(projectId);
			const diffInput = await diffInputContext.diffInput();

			if (!diffInput) {
				showError('Failed to generate commit message', 'No changes found');
				aiIsLoading = false;
				return;
			}

			let firstToken = true;

			const output = await aiService.summarizeCommit({
				diffInput,
				useEmojiStyle: false,
				useBriefStyle: false,
				commitTemplate: prompt,
				branchName: stackSelection?.current?.branchName,
				onToken: (t) => {
					if (firstToken) {
						beginGeneration();
						firstToken = false;
					}
					const updatedText = generatedText + t;
					generatedText = updatedText;
				}
			});

			if (output) {
				generatedText = output;
			}
		} finally {
			aiIsLoading = false;
		}
	}

	let composer = $state<ReturnType<typeof MessageEditor>>();
	let titleInput = $state<HTMLInputElement>();
</script>

<div class="commit-message-wrap">
	<MessageEditorInput
		testId={TestId.CommitDrawerTitleInput}
		bind:ref={titleInput}
		value={title}
		oninput={(e: Event) => {
			const input = e.currentTarget as HTMLInputElement;
			setTitle(input.value);
		}}
		onkeydown={(e: KeyboardEvent) => {
			if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
				e.preventDefault();
				action();
				return;
			}

			if (e.key === 'Enter' || e.key === 'Tab') {
				e.preventDefault();
				composer?.focus();
				return;
			}
		}}
	/>
	<MessageEditor
		testId={TestId.CommitDrawerDescriptionInput}
		bind:this={composer}
		initialValue={description}
		placeholder="Your commit message"
		{projectId}
		{onAiButtonClick}
		{canUseAI}
		{aiIsLoading}
		{suggestionsHandler}
		onChange={(text: string) => {
			setDescription(text);
		}}
		enableFileUpload
		onKeyDown={(e: KeyboardEvent) => {
			if (e.key === 'Tab' && e.shiftKey) {
				e.preventDefault();
				titleInput?.focus();
				return true;
			}

			if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
				e.preventDefault();
				action();
				return true;
			}

			return false;
		}}
	/>
</div>
<EditorFooter {onCancel}>
	<Button
		testId={TestId.CommitDrawerActionButton}
		style="pop"
		onclick={action}
		disabled={disabledAction}
		{loading}
		width={126}>{actionLabel}</Button
	>
</EditorFooter>

<style lang="postcss">
	.commit-message-wrap {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
		gap: 10px;
	}
</style>
