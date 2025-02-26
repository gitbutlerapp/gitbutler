<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';
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
	import { FORMAT_TEXT_COMMAND } from 'lexical';
	import {
		$getSelection as getSelection,
		$createParagraphNode as createParagraphNode
	} from 'lexical';
	import { getContext } from 'svelte';
	import { getEditor } from 'svelte-lexical';
	import type { Writable } from 'svelte/store';

	let optionsRefWidth = $state(0);
	let isScrollToSecondGroup = $state(false);

	const editor = getEditor();
	const isBold: Writable<boolean> = getContext('isBold');
	const isItalic: Writable<boolean> = getContext('isItalic');
	const isUnderline: Writable<boolean> = getContext('isUnderline');
	const isStrikethrough: Writable<boolean> = getContext('isStrikethrough');
	const isCode: Writable<boolean> = getContext('isCode');
	const isLink: Writable<boolean> = getContext('isLink');

	const blockMap = {
		h1: () => createHeadingNode('h1'),
		h2: () => createHeadingNode('h2'),
		h3: () => createHeadingNode('h3'),
		paragraph: createParagraphNode,
		quote: createQuoteNode
	};

	function formatBlock(type: keyof typeof blockMap) {
		editor.update(() => {
			const selection = getSelection();
			setBlocksType(selection, () => blockMap[type]());
		});
	}

	const blockType: Writable<keyof typeof blockMap> = getContext('blockType');
</script>

<div class="formatting-popup">
	<div class="formatting__options" style:width="{optionsRefWidth}px">
		<div class="formatting__options-wrap" class:scrolled={isScrollToSecondGroup}>
			<div class="formatting__group" bind:clientWidth={optionsRefWidth}>
				<Button
					kind="solid"
					size="tag"
					icon="text-bold"
					activated={$isBold}
					onclick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'bold')}
					tooltip={'Bold'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="text-italic"
					activated={$isItalic}
					onclick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'italic')}
					tooltip={'Italic'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="text-underline"
					activated={$isUnderline}
					onclick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'underline')}
					tooltip={'Underline'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="text-strikethrough"
					activated={$isStrikethrough}
					onclick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'strikethrough')}
					tooltip={'Strikethrough'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="text-code"
					activated={$isCode}
					onclick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'code')}
					tooltip={'Code'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="text-quote"
					activated={$blockType === 'quote'}
					onclick={() => formatBlock('quote')}
					tooltip={'Quote'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="text-link"
					activated={$isLink}
					onclick={() => editor.dispatchCommand(TOGGLE_LINK_COMMAND, '')}
					tooltip={'Link'}
				/>
			</div>
			<div class="formatting__group">
				<Button
					kind="solid"
					size="tag"
					icon="text"
					activated={$blockType === 'paragraph'}
					onclick={() => formatBlock('paragraph')}
					tooltip={'Normal text'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="text-h1"
					activated={$blockType === 'h1'}
					onclick={() => formatBlock('h1')}
					tooltip={'Heading 1'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="text-h2"
					activated={$blockType === 'h2'}
					onclick={() => formatBlock('h2')}
					tooltip={'Heading 2'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="text-h3"
					activated={$blockType === 'h3'}
					onclick={() => formatBlock('h3')}
					tooltip={'Heading 3'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="bullet-list"
					onclick={() => editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND, undefined)}
					tooltip={'Unordered list'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="number-list"
					onclick={() => editor.dispatchCommand(INSERT_ORDERED_LIST_COMMAND, undefined)}
					tooltip={'Ordered list'}
				/>
				<Button
					kind="solid"
					size="tag"
					icon="checklist"
					onclick={() => editor.dispatchCommand(INSERT_CHECK_LIST_COMMAND, undefined)}
					tooltip={'Check list'}
				/>
			</div>
		</div>
	</div>
	<div class="formatting__next">
		<Button
			kind="solid"
			size="tag"
			tooltip="More options"
			tooltipDelay={1200}
			icon={isScrollToSecondGroup ? 'chevron-left' : 'chevron-right'}
			onclick={() => {
				isScrollToSecondGroup = !isScrollToSecondGroup;
			}}
		/>
	</div>
</div>

<style lang="postcss">
	.formatting-popup {
		display: flex;
		border-radius: var(--radius-ml);
		background-color: var(--clr-theme-ntrl-element);
		box-shadow: var(--shadow-m);
		border: 1px solid var(--clr-border-2);
		margin: 10px;
		box-shadow: var(--fx-shadow-m);
		width: fit-content;
	}

	.formatting__group,
	.formatting__next {
		display: flex;
		gap: 2px;
		padding: 6px;
		width: fit-content;
	}

	.formatting__next {
		position: relative;
		&:before {
			position: absolute;
			top: 0;
			left: 0;
			content: '';
			display: block;
			width: 1px;
			height: 100%;
			background-color: var(--clr-border-2);
			opacity: 0.3;
		}
	}

	.formatting__options {
		position: relative;
		overflow: hidden;
		flex-wrap: nowrap;
	}

	.formatting__options-wrap {
		display: flex;
		transition: transform 0.18s ease;

		&.scrolled {
			transform: translateX(calc(-100% - 1px));
		}
	}
</style>
