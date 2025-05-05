<script lang="ts">
	import { getEditor } from '$lib/richText/context';
	import { getSelectionPosition } from '$lib/richText/selection';
	import { clickOutside } from '$lib/utils/clickOutside';
	import { portal } from '$lib/utils/portal';
	import {
		COMMAND_PRIORITY_CRITICAL,
		KEY_ARROW_DOWN_COMMAND,
		KEY_ARROW_UP_COMMAND,
		KEY_ENTER_COMMAND,
		KEY_ESCAPE_COMMAND
	} from 'lexical';
	import { fly } from 'svelte/transition';
	import type { EmojiInfo } from '$lib/emoji/utils';

	type Props = {
		suggestedEmojis: EmojiInfo[] | undefined;
		selectSuggestion: (suggestion: EmojiInfo) => void;
		exit: () => void;
	};

	const editor = getEditor();

	const { suggestedEmojis, selectSuggestion, exit }: Props = $props();

	// Top left corner of selection box.
	let position: { left: number; top: number } | undefined = $state();
	// Height of the menu element.
	let offsetHeight = $state(0);
	let selectedSuggestionIndex = $state<number>();

	function scrollSuggestionIntoView(index: number) {
		const suggestionItem = document.getElementById(`emoji-suggestion__item-${index}`);
		if (suggestionItem) {
			suggestionItem.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
		}
	}

	function onArrowUp(event: KeyboardEvent): boolean {
		if (suggestedEmojis === undefined) return false;

		if (selectedSuggestionIndex === undefined) {
			selectedSuggestionIndex = suggestedEmojis.length - 1;
			event.preventDefault();
			event.stopPropagation();
			return true;
		}

		selectedSuggestionIndex =
			(selectedSuggestionIndex - 1 + suggestedEmojis.length) % suggestedEmojis.length;

		scrollSuggestionIntoView(selectedSuggestionIndex);
		event.preventDefault();
		event.stopPropagation();
		return true;
	}

	function onArrowDown(event: KeyboardEvent): boolean {
		if (suggestedEmojis === undefined) return false;

		if (selectedSuggestionIndex === undefined) {
			selectedSuggestionIndex = 0;
			event.preventDefault();
			event.stopPropagation();
			return true;
		}

		selectedSuggestionIndex = (selectedSuggestionIndex + 1) % suggestedEmojis.length;

		scrollSuggestionIntoView(selectedSuggestionIndex);
		event.preventDefault();
		event.stopPropagation();
		return true;
	}

	function onEnter(event: KeyboardEvent): boolean {
		if (suggestedEmojis === undefined || selectedSuggestionIndex === undefined) return false;

		selectSuggestion(suggestedEmojis[selectedSuggestionIndex]);
		event.preventDefault();
		event.stopPropagation();
		return true;
	}

	function onExit(event: KeyboardEvent): boolean {
		position = undefined;
		exit();
		event.preventDefault();
		event.stopPropagation();
		return true;
	}

	let windowScrollY = $state(window.scrollY);
	const selectionPosition = $derived(getSelectionPosition(windowScrollY));

	$effect(() => {
		if (suggestedEmojis !== undefined && suggestedEmojis.length > 0) {
			position = selectionPosition;
		}
	});

	$effect(() => {
		if (suggestedEmojis !== undefined && suggestedEmojis.length > 0) {
			selectedSuggestionIndex = 0;

			const unregisterArrowUp = editor.registerCommand(
				KEY_ARROW_UP_COMMAND,
				onArrowUp,
				COMMAND_PRIORITY_CRITICAL
			);

			const unregisterArrowDown = editor.registerCommand(
				KEY_ARROW_DOWN_COMMAND,
				onArrowDown,
				COMMAND_PRIORITY_CRITICAL
			);

			const unregisterEnter = editor.registerCommand(
				KEY_ENTER_COMMAND,
				onEnter,
				COMMAND_PRIORITY_CRITICAL
			);

			const unregisterEscape = editor.registerCommand(
				KEY_ESCAPE_COMMAND,
				onExit,
				COMMAND_PRIORITY_CRITICAL
			);

			return () => {
				unregisterArrowUp();
				unregisterArrowDown();
				unregisterEnter();
				unregisterEscape();
			};
		}
		selectedSuggestionIndex = undefined;
		position = undefined;
	});
</script>

<svelte:window bind:scrollY={windowScrollY} />

{#if position && suggestedEmojis !== undefined}
	<div
		class="floating-popup hide-native-scrollbar"
		style:left={position.left + 'px'}
		style:top={position.top - offsetHeight - 6 + 'px'}
		bind:offsetHeight
		use:portal={'body'}
		use:clickOutside={{
			handler: () => {
				position = undefined;
				exit();
			}
		}}
		transition:fly={{ y: 5, duration: 120 }}
	>
		<ul class="emoji-suggestion__list">
			{#each suggestedEmojis as emoji, idx}
				<li role="listitem">
					<button
						type="button"
						onclick={() => selectSuggestion(emoji)}
						id="emoji-suggestion__item-{idx}"
						class="emoji-suggestion__item"
						class:selected={idx === selectedSuggestionIndex}
					>
						<p class="text-13">{emoji.unicode}</p>
						<p class="text-13 emoji-sussestion__name">{emoji.label}</p>
					</button>
				</li>
			{/each}
		</ul>
	</div>
{/if}

<style lang="postcss">
	.floating-popup {
		display: flex;
		position: absolute;
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
		box-shadow: var(--shadow-m);
		border: 1px solid var(--clr-border-2);
		box-shadow: var(--fx-shadow-m);
		width: fit-content;
		box-shadow: 0px 4px 14px 0px rgba(0, 0, 0, 0.06);
		overflow-y: auto;
		z-index: var(--z-ground);
	}

	.emoji-suggestion__list {
		padding: 6px;
		display: flex;
		flex-direction: column;
		gap: 4px;

		max-height: 100px;

		&::-webkit-scrollbar {
			display: none;
		}
	}

	.emoji-suggestion__item {
		width: 100%;
		padding: 4px;
		display: flex;
		align-items: center;
		gap: 16px;

		&.selected {
			border-radius: var(--radius-m);
			background: var(--clr-bg-1-muted);
		}
	}

	.emoji-sussestion__name {
		opacity: 0.5;
	}
</style>
