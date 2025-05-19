<script lang="ts">
	import CommitSuggestionsPlugin from '$components/v3/editor/CommitSuggestionsPlugin.svelte';
	import EditorFooter from '$components/v3/editor/EditorFooter.svelte';
	import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
	import MessageEditorInput from '$components/v3/editor/MessageEditorInput.svelte';
	import CommitSuggestions from '$components/v3/editor/commitSuggestions.svelte';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { splitMessage } from '$lib/utils/commitMessage';
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
		initialTitle?: string;
		initialMessage?: string;
		existingCommitId?: string;
	};

	const {
		projectId,
		stackId,
		actionLabel,
		action,
		onCancel,
		disabledAction,
		loading,
		initialTitle,
		initialMessage,
		existingCommitId
	}: Props = $props();

	const uiState = getContext(UiState);
	const aiService = getContext(AIService);
	const promptService = getContext(PromptService);

	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);
	const projectState = $derived(uiState.project(projectId));
	const titleText = $derived(projectState.commitTitle);
	const descriptionText = $derived(projectState.commitDescription);
	const stackSelection = $derived(stackState?.selection);

	const effectiveTitleValue = $derived(titleText.current || (initialTitle ?? ''));
	const effectiveDescriptionValue = $derived(descriptionText.current || (initialMessage ?? ''));

	const suggestionsHandler = new CommitSuggestions(aiService, uiState);
	let commitSuggestionsPlugin = $state<ReturnType<typeof CommitSuggestionsPlugin>>();

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
			titleText.set(title);
			descriptionText.set(description);
			composer?.setText(description);
		}
	});

	function beginGeneration() {
		titleText.set('');
		descriptionText.set('');
		generatedText = '';
	}

	async function onAiButtonClick() {
		if (aiIsLoading || !commitSuggestionsPlugin) return;

		suggestionsHandler.clear();
		aiIsLoading = true;
		await tick();
		try {
			const prompt = promptService.selectedCommitPrompt(projectId);
			const diffInput = commitSuggestionsPlugin.getDiffInput();

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

	export function getMessage() {
		if (effectiveDescriptionValue) {
			return effectiveTitleValue + '\n\n' + effectiveDescriptionValue;
		}
		return effectiveTitleValue;
	}
</script>

<CommitSuggestionsPlugin
	bind:this={commitSuggestionsPlugin}
	{projectId}
	{canUseAI}
	{suggestionsHandler}
	{existingCommitId}
/>

<div class="commit-message-wrap">
	<MessageEditorInput
		testId={TestId.CommitDrawerTitleInput}
		bind:ref={titleInput}
		value={effectiveTitleValue}
		oninput={(e: Event) => {
			const input = e.currentTarget as HTMLInputElement;
			titleText.current = input.value;
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
		initialValue={effectiveDescriptionValue}
		placeholder="Your commit message"
		{projectId}
		{onAiButtonClick}
		{canUseAI}
		{aiIsLoading}
		{suggestionsHandler}
		onChange={(text: string) => {
			descriptionText.current = text;
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
