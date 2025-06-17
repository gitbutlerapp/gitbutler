<script lang="ts">
	import EditorFooter from '$components/v3/editor/EditorFooter.svelte';
	import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
	import MessageEditorInput from '$components/v3/editor/MessageEditorInput.svelte';
	import CommitSuggestions from '$components/v3/editor/commitSuggestions.svelte';
	import DiffInputContext, { type DiffInputContextArgs } from '$lib/ai/diffInputContext.svelte';
	import AIMacros from '$lib/ai/macros.svelte';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
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
		action: (args: { title: string; description: string }) => void;
		onChange?: (args: { title?: string; description?: string }) => void;
		onCancel: (args: { title: string; description: string }) => void;
		disabledAction?: boolean;
		loading?: boolean;
		existingCommitId?: string;
		title: string;
		description: string;
	};

	let {
		projectId,
		stackId,
		actionLabel,
		action,
		onChange,
		onCancel,
		disabledAction,
		loading,
		title,
		description,
		existingCommitId
	}: Props = $props();

	const uiState = getContext(UiState);
	const aiService = getContext(AIService);
	const promptService = getContext(PromptService);

	const worktreeService = getContext(WorktreeService);
	const diffService = getContext(DiffService);
	const uncommittedService = getContext(UncommittedService);
	const stackService = getContext(StackService);

	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);
	const stackSelection = $derived(stackState?.selection);

	const suggestionsHandler = new CommitSuggestions(aiService, uiState);
	const diffInputArgs = $derived<DiffInputContextArgs>(
		existingCommitId
			? { type: 'commit', projectId, commitId: existingCommitId }
			: { type: 'change-selection', projectId, uncommittedService, stackId }
	);
	const diffInputContext = $derived(
		new DiffInputContext(worktreeService, diffService, stackService, diffInputArgs)
	);
	const aiMacros = $derived(new AIMacros(projectId, aiService, promptService, diffInputContext));

	// AI things
	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));
	let aiIsLoading = $state(false);
	const canUseAI = $derived(aiMacros.canUseAI);

	$effect(() => {
		aiMacros.setGenAIEnabled($aiGenEnabled);
	});

	let generatedText = $state<string>('');

	$effect(() => {
		if (generatedText) {
			const { title, description } = splitMessage(generatedText);
			if (titleInput) titleInput.value = title;
			composer?.setText(description);
		}
	});

	function beginGeneration() {
		if (titleInput) titleInput.value = '';
		composer?.setText('');
		generatedText = '';
	}

	async function onAiButtonClick() {
		if (aiIsLoading) return;

		suggestionsHandler.clear();
		aiIsLoading = true;
		await tick();
		try {
			let firstToken = true;

			const output = await aiMacros.generateCommitMessage({
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

	function getTitle() {
		return titleInput?.value || '';
	}

	async function getDescription() {
		return (await composer?.getPlaintext()) || '';
	}

	async function emitAction() {
		const newTitle = getTitle();
		const newDescription = await getDescription();
		action({ title: newTitle, description: newDescription });
	}

	async function handleCancel() {
		const newTitle = getTitle();
		const newDescription = await getDescription();
		onCancel({ title: newTitle, description: newDescription });
	}
</script>

<div class="commit-message-wrap">
	<MessageEditorInput
		testId={TestId.CommitDrawerTitleInput}
		bind:ref={titleInput}
		value={title}
		placeholder="Commit title"
		onchange={(value) => onChange?.({ title: value })}
		onkeydown={async (e: KeyboardEvent) => {
			if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
				e.preventDefault();
				emitAction();
			}

			if (e.key === 'Enter' || (e.key === 'Tab' && !e.shiftKey)) {
				e.preventDefault();
				composer?.focus();
			}
		}}
	/>
	<MessageEditor
		testId={TestId.CommitDrawerDescriptionInput}
		bind:this={composer}
		initialValue={description}
		placeholder="Your commit message"
		enableRuler
		{projectId}
		{onAiButtonClick}
		{canUseAI}
		{aiIsLoading}
		{suggestionsHandler}
		onChange={(text: string) => {
			onChange?.({ description: text });
		}}
		onKeyDown={(e: KeyboardEvent) => {
			if (e.key === 'Tab' && e.shiftKey) {
				e.preventDefault();
				titleInput?.focus();
				return true;
			}

			if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
				e.preventDefault();
				emitAction();
				return true;
			}

			return false;
		}}
	/>
</div>

<EditorFooter onCancel={handleCancel}>
	<Button
		testId={TestId.CommitDrawerActionButton}
		style="pop"
		onclick={emitAction}
		disabled={disabledAction}
		{loading}
		wide>{actionLabel}</Button
	>
</EditorFooter>

<style lang="postcss">
	.commit-message-wrap {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}
</style>
