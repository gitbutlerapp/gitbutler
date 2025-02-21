<script lang="ts">
	import EditorFooter from './editor/EditorFooter.svelte';
	import EditorHeader from './editor/EditorHeader.svelte';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { showError } from '$lib/notifications/toasts';
	import { stackPath } from '$lib/routes/routes.svelte';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { standardConfig } from '$lib/textEditor/config/config';
	import { standardTheme } from '$lib/textEditor/config/theme';
	import { emojiTextNodeTransform } from '$lib/textEditor/plugins/emojiPlugin';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import {
		$convertToMarkdownString as convertToMarkdownString,
		$convertFromMarkdownString as convertFromMarkdownString
	} from '@lexical/markdown';
	import {
		$createParagraphNode as createParagraphNode,
		$createTextNode as createTextNode,
		$getRoot as getRoot,
		TextNode
	} from 'lexical';
	import { onMount } from 'svelte';
	import {
		Composer,
		ContentEditable,
		RichTextPlugin,
		SharedHistoryPlugin,
		ListPlugin,
		CheckListPlugin,
		AutoFocusPlugin,
		PlaceHolder,
		HashtagPlugin,
		PlainTextPlugin,
		AutoLinkPlugin,
		FloatingLinkEditorPlugin,
		CodeHighlightPlugin,
		CodeActionMenuPlugin,
		MarkdownShortcutPlugin,
		ALL_TRANSFORMERS
	} from 'svelte-lexical';
	import { goto } from '$app/navigation';

	const { projectId, stackId }: { projectId: string; stackId: string } = $props();

	const baseBranchService = getContext(BaseBranchService);
	const stackService = getContext(StackService);
	const base = $derived(baseBranchService.base);

	const changeSelection = getContext(ChangeSelectionService);
	const selection = $derived(changeSelection.list().current);

	/**
	 * The stackId parameter is currently optional, mainly so that we don't
	 *
	 * TODO: Figure out if we can show markdown rendered placeholder text.
	 */
	const placeholder = 'Your commit summary';

	/**
	 * Instance of the lexical composer, used for manipulating the contents of the editor
	 * programatically.
	 */
	let composer: Composer;

	/**
	 * Toggles use of markdown on/off in the message editor.
	 */
	let useMarkdown = persisted(true, 'useMarkdown__' + projectId);

	/** Standard configuration for our commit message editor. */
	const initialConfig = standardConfig({
		theme: standardTheme,
		onError: (error: unknown) => {
			showError('Editor error', error);
		}
	});

	/**
	 * Commit message placeholder text.
	 *
	 * TODO: Make stackId required.
	 */
	const branch = $derived(
		stackId ? stackService.getBranchByIndex(projectId, stackId, 0).current : undefined
	);

	/**
	 * TODO: Find a better way of accessing top commit.
	 */
	const commit = $derived(
		branch && branch.data?.state.type === 'Stacked'
			? branch.data.state.subject.localAndRemote.at(0)
			: undefined
	);

	/**
	 * At the moment this code can only commit to the tip of the stack.
	 *
	 * TODO: Implement according to design.
	 */
	const commitParent = $derived(commit ? commit.id : $base?.baseSha);

	/**
	 * TODO: Is there a way of getting the value synchronously?
	 */
	function createCommit() {
		getPlaintext((message) => {
			try {
				_createCommit(message);
			} catch (err: unknown) {
				showError('Failed to commit', err);
			}
		});
	}

	function _createCommit(message: string) {
		stackService.createCommit(projectId, {
			stackId,
			parentId: commitParent!,
			message: message,
			worktreeChanges: selection.map((item) =>
				item.type === 'full'
					? {
							pathBytes: item.pathBytes,
							previousPathBytes: item.previousPathBytes,
							hunkHeaders: []
						}
					: {
							pathBytes: item.pathBytes,
							hunkHeaders: item.hunks
						}
			)
		});
		goto(stackPath(projectId, stackId));
	}

	onMount(() => {
		const unlistenEmoji = composer
			.getEditor()
			.registerNodeTransform(TextNode, emojiTextNodeTransform);
		return () => {
			unlistenEmoji();
		};
	});

	let editorDiv: HTMLDivElement | undefined = $state();

	$effect(() => {
		const editor = composer.getEditor();
		if ($useMarkdown) {
			editor.update(() => {
				convertFromMarkdownString(getRoot().getTextContent(), ALL_TRANSFORMERS);
			});
		} else {
			getPlaintext((text) => {
				editor.update(() => {
					const root = getRoot();
					root.clear();
					const paragraph = createParagraphNode();
					paragraph.append(createTextNode(text));
					root.append(paragraph);
				});
			});
		}
	});

	function getPlaintext(callback: (text: string) => void) {
		const editor = composer.getEditor();
		const state = editor.getEditorState();
		state.read(() => {
			const markdown = convertToMarkdownString(ALL_TRANSFORMERS);
			callback(markdown);
		});
	}
</script>

<div class="new-commit">
	<EditorHeader title="New commit" onRichTextEditorSwitch={(bool) => ($useMarkdown = bool)} />
	<Composer {initialConfig} bind:this={composer}>
		<div class="editor-container" bind:this={editorDiv}>
			<div class="editor-scroller">
				<div class="editor">
					<ContentEditable />
					<PlaceHolder>{placeholder}</PlaceHolder>
				</div>
			</div>

			{#if $useMarkdown}
				<AutoFocusPlugin />
				<AutoLinkPlugin />
				<CheckListPlugin />
				<CodeActionMenuPlugin anchorElem={editorDiv} />
				<CodeHighlightPlugin />
				<FloatingLinkEditorPlugin anchorElem={editorDiv} />
				<HashtagPlugin />
				<ListPlugin />
				<MarkdownShortcutPlugin transformers={ALL_TRANSFORMERS} />
				<RichTextPlugin />
				<SharedHistoryPlugin />
			{:else}
				<PlainTextPlugin />
			{/if}
		</div>
	</Composer>
	<EditorFooter
		SubmitButtonLabel="Create commit!"
		onSubmit={createCommit}
		onCancel={() => goto(stackPath(projectId, stackId))}
	/>
</div>

<style>
	.new-commit {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		height: 100%;
		background: var(--clr-bg-1);
	}

	.editor-container {
		flex-grow: 1;
		background-color: var(--clr-bg-1);
		position: relative;
		display: block;
	}

	.editor-scroller {
		height: 100%;
	}
</style>
