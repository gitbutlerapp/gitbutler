<script lang="ts">
	import Textarea from '@gitbutler/ui/Textarea.svelte';

	interface Props {
		autofocus?: boolean;

		value: string;
		disabled?: boolean;
		oninput?: (e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => void;
		onEmpty?: () => void;
	}
	const { autofocus, value, disabled = false, oninput, onEmpty }: Props = $props();

	let textareaEl: HTMLDivElement | undefined = $state();
</script>

<div class="branch-description-input">
	<Textarea
		bind:textBoxEl={textareaEl}
		{autofocus}
		class="text-12 text-body"
		{value}
		{disabled}
		{oninput}
		flex="1"
		fontSize={12}
		placeholder="Series description"
		unstyled
		padding={{ top: 0, right: 0, bottom: 0, left: 0 }}
		onkeydown={(e: KeyboardEvent & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
			if (e.key === 'Escape') {
				textareaEl?.blur();

				if (value === '') {
					onEmpty?.();
				}
			}
		}}
	/>
</div>

<style>
	.branch-description-input {
		position: relative;
		display: flex;
		flex-direction: column;
		color: var(--clr-text-2);
		padding: 2px;
		border: 1px solid transparent;
		border-radius: var(--radius-s);
		width: calc(100% + 4px);
		margin-left: -2px;
		margin-bottom: -2px;
		transition:
			border-color var(--transition-fast),
			background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}

		&:focus-within {
			background-color: var(--clr-bg-1-muted);
			border-color: var(--clr-border-2);
		}
	}
</style>
