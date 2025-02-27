<script lang="ts">
	import FormattingButton from './FormattingButton.svelte';
	import { getFormatterPosition } from '$lib/textEditor/selection';
	import Button from '@gitbutler/ui/Button.svelte';
	import { clickOutside } from '@gitbutler/ui/utils/clickOutside';
	import { portal } from '@gitbutler/ui/utils/portal';
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
	import { fly } from 'svelte/transition';
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
	const blockType: Writable<keyof typeof blockMap> = getContext('blockType');

	const blockMap = {
		h1: () => createHeadingNode('h1'),
		h2: () => createHeadingNode('h2'),
		h3: () => createHeadingNode('h3'),
		paragraph: createParagraphNode,
		quote: createQuoteNode
	};

	// Top left corner of selection box.
	let position: { left: number; top: number } | undefined = $state();
	// Height of the menu element.
	let offsetHeight = $state(0);

	function formatBlock(type: keyof typeof blockMap) {
		editor.update(() => {
			const selection = getSelection();
			setBlocksType(selection, () => blockMap[type]());
		});
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
		if (nonZeroRange()) {
			position = getFormatterPosition();
		} else {
			position = undefined;
		}
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
</script>

{#if position}
	<div
		class="formatting-popup"
		style:left={position.left + 'px'}
		style:top={position.top - offsetHeight - 6 + 'px'}
		bind:offsetHeight
		use:portal={'body'}
		use:clickOutside={{ handler: () => (position = undefined) }}
		transition:fly={{ y: 5, duration: 120 }}
	>
		<div class="formatting__options" style:width="{optionsRefWidth}px">
			<div class="formatting__options-wrap" class:scrolled={isScrollToSecondGroup}>
				<div class="formatting__group" bind:clientWidth={optionsRefWidth}>
					<FormattingButton
						iconName="text-bold"
						isActivated={$isBold}
						tooltip="Bold"
						onClick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'bold')}
					/>
					<FormattingButton
						iconName="text-italic"
						isActivated={$isItalic}
						tooltip="Italic"
						onClick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'italic')}
					/>
					<FormattingButton
						iconName="text-underline"
						isActivated={$isUnderline}
						tooltip="Underline"
						onClick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'underline')}
					/>
					<FormattingButton
						iconName="text-strikethrough"
						isActivated={$isStrikethrough}
						tooltip="Strikethrough"
						onClick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'strikethrough')}
					/>
					<FormattingButton
						iconName="text-code"
						isActivated={$isCode}
						tooltip="Code"
						onClick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'code')}
					/>
					<FormattingButton
						iconName="text-quote"
						isActivated={$blockType === 'quote'}
						tooltip="Quote"
						onClick={() => formatBlock('quote')}
					/>
					<FormattingButton
						iconName="text-link"
						isActivated={$isLink}
						tooltip="Link"
						onClick={() => editor.dispatchCommand(TOGGLE_LINK_COMMAND, '')}
					/>
				</div>
				<div class="formatting__group">
					<FormattingButton
						iconName="text"
						isActivated={$blockType === 'paragraph'}
						tooltip="Normal text"
						onClick={() => formatBlock('paragraph')}
					/>
					<FormattingButton
						iconName="text-h1"
						isActivated={$blockType === 'h1'}
						tooltip="Heading 1"
						onClick={() => formatBlock('h1')}
					/>
					<FormattingButton
						iconName="text-h2"
						isActivated={$blockType === 'h2'}
						tooltip="Heading 2"
						onClick={() => formatBlock('h2')}
					/>
					<FormattingButton
						iconName="text-h3"
						isActivated={$blockType === 'h3'}
						tooltip="Heading 3"
						onClick={() => formatBlock('h3')}
					/>
					<FormattingButton
						iconName="bullet-list"
						tooltip="Unordered list"
						onClick={() => editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND, undefined)}
					/>
					<FormattingButton
						iconName="number-list"
						tooltip="Ordered list"
						onClick={() => editor.dispatchCommand(INSERT_ORDERED_LIST_COMMAND, undefined)}
					/>
					<FormattingButton
						iconName="checklist"
						tooltip="Check list"
						onClick={() => editor.dispatchCommand(INSERT_CHECK_LIST_COMMAND, undefined)}
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
{/if}

<style lang="postcss">
	.formatting-popup {
		display: flex;
		position: absolute;
		border-radius: var(--radius-ml);
		background-color: var(--clr-theme-ntrl-element);
		box-shadow: var(--shadow-m);
		border: 1px solid var(--clr-border-2);
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
