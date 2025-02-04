<script lang="ts" module>
	export type Range = {
		from: number;
		to: number;
	};
	// See `addAttributes` below
	export interface MentionNodeAttrs {
		/**
		 * The identifier for the selected item that was mentioned, stored as a `data-id`
		 * attribute.
		 */
		id: string | null;
		/**
		 * The label to be rendered by the editor as the displayed text for this mentioned
		 * item, if provided. Stored as a `data-label` attribute. See `renderLabel`.
		 */
		label?: string | null;
	}

	export interface SuggestionProps {
		/**
		 * The editor instance.
		 */
		editor: Editor;

		/**
		 * The range of the suggestion.
		 */
		range: Range;

		/**
		 * The current suggestion query.
		 */
		query: string;

		/**
		 * The current suggestion text.
		 */
		text: string;

		/**
		 * The suggestion items array.
		 */
		items: string[];

		/**
		 * A function that is called when a suggestion is selected.
		 * @param props The props object.
		 * @returns void
		 */
		command: (props: MentionNodeAttrs) => void;

		/**
		 * The decoration node HTML element
		 * @default null
		 */
		decorationNode: Element | null;

		/**
		 * The function that returns the client rect
		 * @default null
		 * @example () => new DOMRect(0, 0, 0, 0)
		 */
		clientRect?: (() => DOMRect | null) | null;
	}
</script>

<script lang="ts">
	import { Editor } from '@tiptap/core';
	import Document from '@tiptap/extension-document';
	import Mention from '@tiptap/extension-mention';
	import Paragraph from '@tiptap/extension-paragraph';
	import Text from '@tiptap/extension-text';

	interface Props {
		getSuggestionItems: (query: string) => Promise<string[]>;
		onSuggestionStart: (props: SuggestionProps) => void;
		onSuggestionUpdate: (props: SuggestionProps) => void;
		onSuggestionExit: (props: SuggestionProps) => void;
		onSuggestionKeyDown: (event: KeyboardEvent) => boolean;
		onKeyDown?: (event: KeyboardEvent) => boolean;
		onTextUpdate?: (text: string) => void;
	}

	const {
		getSuggestionItems,
		onSuggestionStart,
		onSuggestionUpdate,
		onSuggestionKeyDown,
		onSuggestionExit,
		onKeyDown,
		onTextUpdate
	}: Props = $props();

	let element = $state<HTMLDivElement>();
	let editor = $state<Editor>();

	$effect(() => {
		editor = new Editor({
			element: element,
			editorProps: {
				handleKeyDown(_, event) {
					if (onKeyDown) {
						return onKeyDown(event);
					}
					return false;
				}
			},
			extensions: [
				Document,
				Paragraph,
				Text,
				Mention.configure({
					HTMLAttributes: {
						class: 'mention'
					},
					suggestion: {
						items: async ({ query }): Promise<string[]> => {
							return await getSuggestionItems(query);
						},
						render: () => {
							return {
								onStart: (props) => {
									onSuggestionStart(props);
								},
								onUpdate: (props) => {
									onSuggestionUpdate(props);
								},
								onKeyDown: (props) => {
									return onSuggestionKeyDown(props.event);
								},
								onExit: (props) => {
									const range = {
										from: props.editor.state.selection.from,
										to: props.editor.state.selection.from + props.query.length
									};
									props.editor.commands.setTextSelection(range);
									props.editor.commands.deleteSelection();

									onSuggestionExit(props);
								}
							};
						}
					}
				})
			],
			onTransaction: () => {
				// force re-render so `editor.isActive` works as expected
				editor = editor;
			},
			onUpdate: ({ editor }) => {
				onTextUpdate?.(editor.getText());
			}
		});

		return () => {
			editor?.destroy();
		};
	});

	export function getEditor(): Editor | undefined {
		return editor;
	}
</script>

<div
	style:--lineheight-ratio={1.6}
	class="text-body text-13 rich-text-wrapper"
	bind:this={element}
></div>

<style>
	.rich-text-wrapper :global(.mention) {
		padding: 0px 4px;
		border-radius: var(--radius-s);
		background: var(--clr-theme-pop-bg-muted);
		color: var(--clr-theme-pop-on-soft);
	}

	.rich-text-wrapper > :global(.ProseMirror) {
		outline: 0;
		color: var(--clr-text-1);
	}
</style>
