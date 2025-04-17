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
	import { splitMessage } from '$lib/utils/commitMessage';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { tick } from 'svelte';

	type Props = {
		existingCommitId?: string;
		projectId: string;
		stackId?: string;
		actionLabel: string;
		action: () => void;
		onCancel: () => void;
		disabledAction?: boolean;
		loading?: boolean;
		initialTitle?: string;
		initialMessage?: string;
	};

	const {
		existingCommitId,
		projectId,
		stackId,
		actionLabel,
		action,
		onCancel,
		disabledAction,
		loading,
		initialTitle,
		initialMessage
	}: Props = $props();

	const uiState = getContext(UiState);
	const aiService = getContext(AIService);
	const promptService = getContext(PromptService);

	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);
	const projectState = $derived(uiState.project(projectId));
	const titleText = $derived(projectState.commitTitle);
	const descriptionText = $derived(projectState.commitDescription);
	const stackSelection = $derived(stackState?.selection);

	// const useRichText = uiState.global.useRichText;

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

	$effect(() => {
		if (isDefined(initialTitle)) {
			titleText.current = initialTitle;
		}

		if (isDefined(initialMessage)) {
			descriptionText.current = initialMessage;
		}
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
		if (descriptionText.current) {
			return titleText.current + '\n\n' + descriptionText.current;
		}
		return titleText.current;
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
		bind:ref={titleInput}
		value={titleText.current}
		oninput={(e: Event) => {
			const input = e.currentTarget as HTMLInputElement;
			projectState.commitTitle.current = input.value;
		}}
		onkeydown={(e: KeyboardEvent) => {
			if (e.key === 'Enter' || e.key === 'Tab') {
				e.preventDefault();
				composer?.focus();
				return;
			}

			if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
				e.preventDefault();
				action();
				return;
			}
		}}
	/>

	<MessageEditor
		bind:this={composer}
		initialValue={descriptionText.current}
		placeholder={'Your commit message'}
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
	<Button style="pop" onclick={action} disabled={disabledAction} {loading} width={126}
		>{actionLabel}</Button
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
