<script lang="ts">
	import Textarea from '@gitbutler/ui/Textarea.svelte';

	interface Props {
		value?: string;
		disabled?: boolean;
		onEmpty?: () => void;
		onBlur?: (value: string | undefined | null) => void;
		textAreaEl?: HTMLTextAreaElement;
	}

	let {
		value = $bindable(),
		disabled = false,
		onBlur,
		onEmpty,
		textAreaEl = $bindable()
	}: Props = $props();
</script>

<div class="branch-description-input">
	<Textarea
		bind:textBoxEl={textAreaEl}
		class="text-12 text-body"
		{value}
		{disabled}
		flex="1"
		fontSize={12}
		placeholder="Series description"
		unstyled
		padding={{ top: 0, right: 0, bottom: 0, left: 0 }}
		onblur={(e: FocusEvent & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
			onBlur?.(e.currentTarget.value.trim() || null);

			if (e.currentTarget.value === '') {
				onEmpty?.();
			}
		}}
		onkeydown={(e: KeyboardEvent & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
			if (e.key === 'Escape') {
				textAreaEl?.blur();

				if (e.currentTarget.value === '') {
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
		padding: 0 2px;
		border: 1px solid transparent;
		border-radius: var(--radius-s);
		margin-bottom: -5px;

		width: 100%;
		transition:
			border-color var(--transition-fast),
			background-color var(--transition-fast);

		&:not(:focus-within):hover {
			background-color: var(--clr-bg-1-muted);
		}

		&:focus-within {
			background-color: var(--clr-bg-1-muted);

			border: 1px solid var(--clr-border-2);
			/* background-color: var(--clr-bg-2); */
		}
	}
</style>
