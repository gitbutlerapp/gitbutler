<script lang="ts">
	import { AIService } from '$lib/ai/service';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { devLog } from '@gitbutler/shared/logging';
	import { debouncePromise } from '@gitbutler/shared/utils/misc';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
	import Formatter from '@gitbutler/ui/richText/plugins/Formatter.svelte';
	import GhostTextPlugin from '@gitbutler/ui/richText/plugins/GhostText.svelte';
	import GiphyPlugin from '@gitbutler/ui/richText/plugins/GiphyPlugin.svelte';
	import FormattingBar from '@gitbutler/ui/richText/tools/FormattingBar.svelte';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	interface Props {
		modifierPrompt: string | undefined;
		projectId: string;
		stackId: string;
		markdown: boolean;
	}

	let { markdown = $bindable(), stackId, projectId, modifierPrompt }: Props = $props();

	const [aiService, uiState, stackService, idSelection, worktreeService, diffService] = inject(
		AIService,
		UiState,
		StackService,
		IdSelection,
		WorktreeService,
		DiffService
	);

	const stackState = $derived(uiState.stack(stackId));
	const selected = $derived(stackState.selection.get());
	const branchName = $derived(selected.current?.branchName);
	const selection = $derived(idSelection.values());
	const selectionPaths = $derived(
		selection.map((item) => (item.type === 'worktree' ? item.path : undefined)).filter(isDefined)
	);

	const changes = $derived(worktreeService.getChangesById(projectId, selectionPaths));

	type ChangeDiff<T> = {
		path: string;
		diff: T;
	};

	let changeDiffs = $state<ChangeDiff<Awaited<ReturnType<typeof diffService.getDiffDirectly>>>[]>();

	$effect(() => {
		const treeChanges = changes.current.data;
		if (!treeChanges) return;
		Promise.all(
			treeChanges.map((change) =>
				diffService.getDiffDirectly(projectId, change).then((diff) => ({
					path: change.path,
					diff
				}))
			)
		).then((diffs) => {
			devLog('update diffs:');
			changeDiffs = diffs;
		});
	});

	const commitsResponse = $derived(
		branchName ? stackService.commits(projectId, stackId, branchName) : undefined
	);
	const commits = $derived(commitsResponse?.current.data ?? []);
	const commitMessages = $derived(commits.map((commit) => commit.message));

	let composer = $state<ReturnType<typeof RichTextEditor>>();
	let formatter = $state<ReturnType<typeof Formatter>>();
	let ghostTextComponent = $state<ReturnType<typeof GhostTextPlugin>>();

	export async function getPlaintext(): Promise<string | undefined> {
		return composer?.getPlaintext();
	}

	let alwaysAutoComplete = $state(false);

	let editorText = $state<string | undefined>();
	let lastSentMessage = $state<string | undefined>();
	let lasSelectedGhostText = $state<string | undefined>();

	function getCleanHunks() {
		if (!changeDiffs) return [];
		return changeDiffs
			.map(({ diff, path }) => {
				if (!diff.data) return undefined;
				if (diff.data.type !== 'Patch') return undefined;
				return {
					path,
					diff: diff.data.subject.hunks.map((hunk) => hunk.diff)
				};
			})
			.filter(isDefined);
	}

	async function handleChange(text: string) {
		editorText = text;
		devLog('Text changed:', text);
		if (!alwaysAutoComplete) return;

		const hunks = getCleanHunks();

		if (lasSelectedGhostText && text.endsWith(lasSelectedGhostText)) return;
		if (lastSentMessage === text) return;
		if (!text) {
			ghostTextComponent?.reset();
			return;
		}
		lastSentMessage = text;
		devLog('Text changed:', text);
		const autoCompletion = await aiService.autoCompleteCommitMessage({
			currentValue: text,
			stagedChanges: hunks,
			commitMessages,
			modifierPrompt
		});
		devLog('Auto completion:', autoCompletion);
		if (autoCompletion) {
			ghostTextComponent?.setText(autoCompletion);
		}
	}

	async function suggest(text: string) {
		const hunks = getCleanHunks();

		if (lasSelectedGhostText && text.endsWith(lasSelectedGhostText)) return;
		if (lastSentMessage === text) return;
		if (!text) {
			ghostTextComponent?.reset();
			return;
		}
		lastSentMessage = text;
		const autoCompletion = await aiService.autoCompleteCommitMessage({
			currentValue: text,
			stagedChanges: hunks,
			commitMessages,
			modifierPrompt
		});

		devLog('Auto completion:', autoCompletion);
		if (autoCompletion) {
			ghostTextComponent?.setText(autoCompletion);
		}
	}

	function handleKeyDown(event: KeyboardEvent | null): boolean {
		if (alwaysAutoComplete) return false;
		if (!event) return false;
		if (event.key === 'g' && (event.ctrlKey || event.metaKey)) {
			if (editorText) suggest(editorText);
			return true;
		}
		return false;
	}

	const debouncedHandleChange = debouncePromise(handleChange, 500);

	function onSelectGhostText(text: string) {
		devLog('Selected ghost text:', text);
		lasSelectedGhostText = text;
	}
</script>

<div class="editor-wrapper">
	<div class="editor-header">
		<div class="editor-tabs">
			<button
				type="button"
				class="text-13 text-semibold editor-tab"
				class:active={!markdown}
				onclick={() => {
					markdown = false;
				}}>Plain</button
			>
			<button
				type="button"
				class="text-13 text-semibold editor-tab"
				class:active={markdown}
				onclick={() => {
					markdown = true;
				}}>Rich-text Editor</button
			>
		</div>
		<Checkbox bind:checked={alwaysAutoComplete}  />
		<FormattingBar bind:formatter />
	</div>

	<div role="presentation" class="message-editor-wrapper">
		<RichTextEditor
			styleContext="client-editor"
			namespace="CommitMessageEditor"
			placeholder="Your commit message"
			bind:this={composer}
			{markdown}
			onError={(e) => showError('Editor error', e)}
			onChange={debouncedHandleChange}
			onKeyDown={handleKeyDown}
		>
			{#snippet plugins()}
				<Formatter bind:this={formatter} />
				<GiphyPlugin />
				<GhostTextPlugin bind:this={ghostTextComponent} onSelection={onSelectGhostText} />
				<GiphyPlugin />
			{/snippet}
		</RichTextEditor>
	</div>
</div>

<style lang="postcss">
	.editor-wrapper {
		display: flex;
		flex-direction: column;
		flex: 1;
		background-color: var(--clr-bg-1);
	}

	.editor-header {
		position: relative;
		z-index: 1;
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.editor-tabs {
		display: flex;
	}

	.editor-tab {
		position: relative;
		color: var(--clr-text-2);
		padding: 10px;
		background-color: var(--clr-bg-1-muted);
		border: 1px solid transparent;
		border-bottom: none;
		border-radius: var(--radius-m) var(--radius-m) 0 0;
		transition: color var(--transition-fast);

		&.active {
			color: var(--clr-text-1);
			background-color: var(--clr-bg-1);
			border-color: var(--clr-border-2);

			&:after {
				content: '';
				position: absolute;
				bottom: 0;
				left: 0;
				width: 100%;
				height: 1px;
				background-color: var(--clr-border-3);
				transform: translateY(100%);
			}
		}

		&:hover {
			color: var(--clr-text-1);
		}
	}

	.message-editor-wrapper {
		flex: 1;
		border-radius: 0 var(--radius-m) var(--radius-m) var(--radius-m);
		border: 1px solid var(--clr-border-2);
		overflow: hidden;

		&:hover,
		&:focus-within {
			border-color: var(--clr-border-1);
		}
	}
</style>
