<script lang="ts">
	import AiContextMenu from '$components/v3/editor/AIContextMenu.svelte';
	import CommitSuggestions from '$components/v3/editor/commitSuggestions.svelte';
	import { AIService } from '$lib/ai/service';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { debouncePromise } from '@gitbutler/shared/utils/misc';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
	import Formatter from '@gitbutler/ui/richText/plugins/Formatter.svelte';
	import GhostTextPlugin from '@gitbutler/ui/richText/plugins/GhostText.svelte';
	import GiphyPlugin from '@gitbutler/ui/richText/plugins/GiphyPlugin.svelte';
	import FormattingBar from '@gitbutler/ui/richText/tools/FormattingBar.svelte';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	interface Props {
		projectId: string;
		stackId: string;
		markdown: boolean;
		initialValue?: string;
	}

	let { markdown = $bindable(), projectId, initialValue }: Props = $props();

	const [aiService, idSelection, worktreeService, diffService] = inject(
		AIService,
		IdSelection,
		WorktreeService,
		DiffService
	);

	const aiGenEnabled = projectAiGenEnabled(projectId);
	let aiConfigurationValid = $state(false);
	const suggestionsHandler = new CommitSuggestions(aiService);
	const selection = $derived(idSelection.values());
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
		aiService.validateConfiguration().then((valid) => {
			aiConfigurationValid = valid;
		});
	});

	$effect(() => {
		suggestionsHandler.setStagedChanges(changeDiffs);
	});

	const canUseAI = $derived($aiGenEnabled && aiConfigurationValid);

	$effect(() => {
		suggestionsHandler.setCanUseAI(canUseAI);
	});

	let composer = $state<ReturnType<typeof RichTextEditor>>();
	let formatter = $state<ReturnType<typeof Formatter>>();

	export async function getPlaintext(): Promise<string | undefined> {
		return composer?.getPlaintext();
	}

	async function handleChange(text: string) {
		await suggestionsHandler.onChange(text);
	}

	const debouncedHandleChange = debouncePromise(handleChange, 500);

	function handleKeyDown(event: KeyboardEvent | null) {
		return suggestionsHandler.onKeyDown(event);
	}

	let aiButton = $state<HTMLElement>();
	let aiContextMenu = $state<ReturnType<typeof ContextMenu>>();

	function onAiButtonClick(e: MouseEvent) {
		aiButton = e.currentTarget as HTMLElement;
		aiContextMenu?.toggle();
	}
</script>

<AiContextMenu
	bind:menu={aiContextMenu}
	leftClickTrigger={aiButton}
	onTypeSuggestions={suggestionsHandler.suggestOnType}
	toggleOnTypeSuggestions={() => suggestionsHandler.toggleSuggestOnType()}
/>

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
		<FormattingBar bind:formatter {onAiButtonClick} {canUseAI} />
	</div>

	<div role="presentation" class="message-editor-wrapper">
		<RichTextEditor
			styleContext="client-editor"
			namespace="CommitMessageEditor"
			placeholder="Your commit message"
			bind:this={composer}
			{markdown}
			onError={(e) => showError('Editor error', e)}
			initialText={initialValue}
			onChange={debouncedHandleChange}
			onKeyDown={handleKeyDown}
		>
			{#snippet plugins()}
				<Formatter bind:this={formatter} />
				<GiphyPlugin />
				<GhostTextPlugin
					bind:this={suggestionsHandler.ghostTextComponent}
					onSelection={(text) => suggestionsHandler.onAcceptSuggestion(text)}
				/>
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
