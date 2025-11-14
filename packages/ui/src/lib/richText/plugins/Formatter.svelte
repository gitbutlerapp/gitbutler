<script lang="ts" module>
	export type FormatStyle =
		| 'text-bold'
		| 'text-italic'
		| 'text-underline'
		| 'text-strikethrough'
		| 'text-code'
		| 'text-quote'
		| 'text-link'
		| 'text'
		| 'text-h1'
		| 'text-h2'
		| 'text-h3'
		| 'bullet-list'
		| 'number-list'
		| 'checklist';
</script>

<script lang="ts">
	import { TOGGLE_LINK_COMMAND } from '@lexical/link';
	import {
		INSERT_CHECK_LIST_COMMAND,
		INSERT_ORDERED_LIST_COMMAND,
		INSERT_UNORDERED_LIST_COMMAND
	} from '@lexical/list';
	import {
		$createQuoteNode as createQuoteNode,
		$createHeadingNode as createHeadingNode
	} from '@lexical/rich-text';
	import { $setBlocksType as setBlocksType } from '@lexical/selection';
	import {
		$isRangeSelection as isRangeSelection,
		COMMAND_PRIORITY_CRITICAL,
		FORMAT_TEXT_COMMAND,
		SELECTION_CHANGE_COMMAND
	} from 'lexical';
	import {
		$getSelection as getSelection,
		$createParagraphNode as createParagraphNode
	} from 'lexical';
	import { getContext, onMount } from 'svelte';
	import { getEditor } from 'svelte-lexical';
	import type { Writable } from 'svelte/store';

	const editor = getEditor();
	const isBold: Writable<boolean> = getContext('isBold');
	const isItalic: Writable<boolean> = getContext('isItalic');
	const isUnderline: Writable<boolean> = getContext('isUnderline');
	const isStrikethrough: Writable<boolean> = getContext('isStrikethrough');
	const isCode: Writable<boolean> = getContext('isCode');
	const isLink: Writable<boolean> = getContext('isLink');
	const blockType: Writable<keyof typeof blockMap> = getContext('blockType');

	let isNonZeroRange = $state<boolean>();

	const blockMap = {
		h1: () => createHeadingNode('h1'),
		h2: () => createHeadingNode('h2'),
		h3: () => createHeadingNode('h3'),
		paragraph: createParagraphNode,
		quote: createQuoteNode
	};

	function formatBlock(type: keyof typeof blockMap) {
		editor.update(
			() => {
				const selection = getSelection();
				setBlocksType(selection, () => blockMap[type]());
			},
			{ tag: 'history-merge' }
		);
	}

	function nonZeroRange() {
		const selection = getSelection();
		return (
			isRangeSelection(selection) &&
			(selection.anchor.key !== selection.focus.key ||
				selection.anchor.offset !== selection.focus.offset)
		);
	}

	function onSelectionChange() {
		isNonZeroRange = nonZeroRange();
		return true;
	}

	onMount(() => {
		const unregister = editor.registerCommand(
			SELECTION_CHANGE_COMMAND,
			onSelectionChange,
			COMMAND_PRIORITY_CRITICAL
		);
		return unregister;
	});

	export const imports = {
		get isBold() {
			return $isBold;
		},
		get isItalic() {
			return $isItalic;
		},
		get isUnderline() {
			return $isUnderline;
		},
		get isStrikethrough() {
			return $isStrikethrough;
		},
		get isCode() {
			return $isCode;
		},
		get isLink() {
			return $isLink;
		},
		get isQuote() {
			return $blockType === 'quote';
		},
		get isNormal() {
			return $blockType === 'paragraph';
		},
		get isH1() {
			return $blockType === 'h1';
		},
		get isH2() {
			return $blockType === 'h2';
		},
		get isH3() {
			return $blockType === 'h3';
		},
		get blockType() {
			return $blockType;
		},
		get blockMap() {
			return blockMap;
		},
		get isNonZeroRange() {
			return isNonZeroRange;
		}
	};

	export function format(style: FormatStyle): true {
		switch (style) {
			case 'text-bold':
				editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'bold');
				return true;
			case 'text-italic':
				editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'italic');
				return true;
			case 'text-underline':
				editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'underline');
				return true;
			case 'text-strikethrough':
				editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'strikethrough');
				return true;
			case 'text-code':
				editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'code');
				return true;
			case 'text-quote':
				formatBlock('quote');
				return true;
			case 'text-link':
				editor.dispatchCommand(TOGGLE_LINK_COMMAND, '');
				return true;
			case 'text':
				formatBlock('paragraph');
				return true;
			case 'text-h1':
				formatBlock('h1');
				return true;
			case 'text-h2':
				formatBlock('h2');
				return true;
			case 'text-h3':
				formatBlock('h3');
				return true;
			case 'bullet-list':
				editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND, undefined);
				return true;
			case 'number-list':
				editor.dispatchCommand(INSERT_ORDERED_LIST_COMMAND, undefined);
				return true;
			case 'checklist':
				editor.dispatchCommand(INSERT_CHECK_LIST_COMMAND, undefined);
				return true;
		}
	}
</script>
