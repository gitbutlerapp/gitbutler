<script lang="ts">
	import FloatingCommitBox from '$components/FloatingCommitBox.svelte';
	import EditorFooter from '$components/editor/EditorFooter.svelte';
	import MessageEditor, { type AiButtonClickParams } from '$components/editor/MessageEditor.svelte';
	import MessageEditorInput from '$components/editor/MessageEditorInput.svelte';
	import CommitSuggestions from '$components/editor/commitSuggestions.svelte';
	import DiffInputContext, { type DiffInputContextArgs } from '$lib/ai/diffInputContext.svelte';
	import AIMacros from '$lib/ai/macros.svelte';
	import { PROMPT_SERVICE } from '$lib/ai/promptService';
	import { AI_SERVICE } from '$lib/ai/service';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { DIFF_SERVICE } from '$lib/hunks/diffService.svelte';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { WORKTREE_SERVICE } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Button } from '@gitbutler/ui';

	import { tick } from 'svelte';

	type Props = {
		projectId: string;
		stackId?: string;
		actionLabel: string;
		action: (args: { title: string; description: string }) => void;
		onChange?: (args: { title?: string; description?: string }) => void;
		onCancel: (args: { title: string; description: string }) => void;
		noPadding?: boolean;
		disabledAction?: boolean;
		loading?: boolean;
		existingCommitId?: string;
		title: string;
		floatingBoxHeader?: string;
		description: string;
	};

	let {
		projectId,
		stackId,
		actionLabel,
		action,
		onChange,
		onCancel,
		noPadding,
		disabledAction,
		loading,
		title,
		description,
		floatingBoxHeader = 'Create commit',
		existingCommitId
	}: Props = $props();

	const uiState = inject(UI_STATE);
	const aiService = inject(AI_SERVICE);
	const promptService = inject(PROMPT_SERVICE);

	const useFloatingBox = $derived(uiState.global.useFloatingBox);

	const worktreeService = inject(WORKTREE_SERVICE);
	const diffService = inject(DIFF_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const stackService = inject(STACK_SERVICE);

	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);
	const stackSelection = $derived(stackState?.selection);

	let composer = $state<ReturnType<typeof MessageEditor>>();
	let titleInput = $state<HTMLTextAreaElement>();

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
			const newMessage = splitMessage(generatedText);
			title = newMessage.title;
			composer?.setText(newMessage.description);
		}
	});

	function beginGeneration() {
		title = '';
		composer?.setText('');
		generatedText = '';
	}

	async function onAiButtonClick(params: AiButtonClickParams = {}) {
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
				},
				...params
			});

			if (output) {
				generatedText = output;
				const newMessage = splitMessage(generatedText);
				onChange?.({ title: newMessage.title, description: newMessage.description });
			}
		} finally {
			aiIsLoading = false;
		}
	}

	async function getDescription() {
		return (await composer?.getPlaintext()) || '';
	}

	async function emitAction() {
		const newDescription = await getDescription();
		action({ title: title.trim(), description: newDescription.trim() });
	}

	async function handleCancel() {
		const newDescription = await getDescription();
		onCancel({ title: title.trim(), description: newDescription.trim() });
	}

	export function isRichTextMode(): boolean {
		return composer?.isRichTextMode?.() || false;
	}
</script>

{#snippet editorContent()}
	<MessageEditorInput
		testId={TestId.CommitDrawerTitleInput}
		bind:ref={titleInput}
		bind:value={title}
		placeholder="Commit title (required)"
		onchange={(value) => {
			onChange?.({ title: value });
		}}
		onkeydown={async (e: KeyboardEvent) => {
			if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
				e.preventDefault();
				if (title.trim()) {
					emitAction();
				}
			}
			if (e.key === 'Enter' || (e.key === 'Tab' && !e.shiftKey)) {
				e.preventDefault();
				composer?.focus();
			}
			if (e.key === 'Escape') {
				e.preventDefault();
				handleCancel();
			}
		}}
	/>
	<MessageEditor
		testId={TestId.CommitDrawerDescriptionInput}
		bind:this={composer}
		initialValue={description}
		placeholder="Commit message"
		enableRuler
		{projectId}
		{onAiButtonClick}
		{canUseAI}
		{aiIsLoading}
		{suggestionsHandler}
		useRuler={uiState.global.useRuler.current}
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
				if (title.trim()) {
					emitAction();
				}
				return true;
			}
			if (e.key === 'Escape') {
				e.preventDefault();
				handleCancel();
				return true;
			}

			return false;
		}}
	/>
	<EditorFooter onCancel={handleCancel}>
		<Button
			testId={TestId.CommitDrawerActionButton}
			style="pop"
			onclick={emitAction}
			disabled={disabledAction || !title.trim()}
			tooltip={!title.trim() ? 'Commit title is required' : undefined}
			hotkey="⌘↵"
			{loading}
			wide>{actionLabel}</Button
		>
	</EditorFooter>
{/snippet}

{#if useFloatingBox.current}
	<FloatingCommitBox
		title={floatingBoxHeader}
		onExitFloatingModeClick={() => {
			useFloatingBox.set(false);
		}}
	>
		{@render editorContent()}
	</FloatingCommitBox>
{:else}
	<div class="commit-message" class:no-padding={noPadding}>
		{@render editorContent()}
	</div>
{/if}

<style lang="postcss">
	.commit-message {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		background-color: var(--clr-bg-1);

		&:not(.no-padding) {
			padding: 12px;
		}
	}
</style>
