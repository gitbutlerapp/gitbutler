<script lang="ts">
	import EditorFooter from '$components/v3/editor/EditorFooter.svelte';
	import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
	import CommitSuggestions from '$components/v3/editor/commitSuggestions.svelte';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService, type DiffInput } from '$lib/ai/service';
	import { persistedCommitMessage, projectAiGenEnabled } from '$lib/config/config';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { tick } from 'svelte';

	type Props = {
		isNewCommit?: boolean;
		projectId: string;
		stackId: string;
		actionLabel: string;
		action: () => void;
		onCancel: () => void;
		disabledAction?: boolean;
		loading?: boolean;
		initialTitle?: string;
		initialMessage?: string;
	};

	const {
		isNewCommit,
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
	const worktreeService = getContext(WorktreeService);
	const idSelection = getContext(IdSelection);
	const diffService = getContext(DiffService);
	const promptService = getContext(PromptService);

	const titleText = $derived(uiState.project(projectId).commitTitle);
	const descriptionText = $derived(uiState.project(projectId).commitMessage);
	const stackState = $derived(uiState.stack(stackId));
	const stackSelection = $derived(stackState.selection.current);

	const suggestionsHandler = new CommitSuggestions(aiService, uiState);
	const selection = $derived(idSelection.values({ type: 'worktree' }));
	const selectionPaths = $derived(
		selection.map((item) => (item.type === 'worktree' ? item.path : undefined)).filter(isDefined)
	);

	const changes = $derived(worktreeService.getChangesById(projectId, selectionPaths));
	const treeChanges = $derived(changes?.current.data);
	const changeDiffsResponse = $derived(
		treeChanges ? diffService.getChanges(projectId, treeChanges) : undefined
	);
	const changeDiffs = $derived(
		changeDiffsResponse?.current.map((item) => item.data).filter(isDefined) ?? []
	);

	$effect(() => {
		suggestionsHandler.setStagedChanges(changeDiffs);
	});

	$effect(() => {
		suggestionsHandler.setCanUseAI(canUseAI);
	});

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

	const commitMessage = persistedCommitMessage(projectId, stackId);

	$effect(() => {
		if (isNewCommit) {
			$commitMessage = [titleText.current, descriptionText.current].filter((a) => a).join('\n\n');
		}
	});

	function splitTextMessage(generatedMessage: string) {
		const splitText = generatedMessage.split('\n\n');
		const title = splitText[0] ?? '';
		const description = splitText.slice(1).join('\n\n');
		return [title, description] as const;
	}

	let generatedText = $state<string>('');

	$effect(() => {
		if (generatedText) {
			const [title, description] = splitTextMessage(generatedText);
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

	function geDiffInput(): DiffInput[] {
		const diffInput: DiffInput[] = [];

		for (const diff of changeDiffs) {
			const filePath = diff.path;
			const diffStringBuffer: string[] = [];
			if (diff.diff.type !== 'Patch') continue;
			for (const hunk of diff.diff.subject.hunks) {
				diffStringBuffer.push(hunk.diff);
			}

			const diffString = diffStringBuffer.join('\n');
			diffInput.push({
				filePath,
				diff: diffString
			});
		}
		return diffInput;
	}

	async function onAiButtonClick() {
		if (aiIsLoading) return;

		suggestionsHandler.clear();
		aiIsLoading = true;
		await tick();
		try {
			const prompt = promptService.selectedCommitPrompt(projectId);
			const diffInput = geDiffInput();

			let firstToken = true;

			const output = await aiService.summarizeCommit({
				diffInput,
				useEmojiStyle: false,
				useBriefStyle: false,
				commitTemplate: prompt,
				branchName: stackSelection?.branchName,
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
	let titleInput = $state<ReturnType<typeof Textbox>>();

	export function getMessage() {
		return $commitMessage;
	}
</script>

<div class="commit-message-input">
	<Textbox
		bind:this={titleInput}
		autofocus
		size="large"
		placeholder="Commit title"
		value={titleText.current}
		oninput={(value: string) => {
			titleText.set(value);
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
		{stackId}
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
	.commit-message-input {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
		gap: 10px;
	}
</style>
