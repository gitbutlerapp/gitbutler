<script lang="ts">
	import Textarea from '@gitbutler/ui/Textarea.svelte';

	interface Props {
		autofocus?: boolean;
		class: string;
		value: string;
		disabled?: boolean;
		oninput?: (e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => void;
		onEmpty?: () => void;
	}
	const {
		autofocus,
		value,
		class: className = '',
		disabled = false,
		oninput,
		onEmpty
	}: Props = $props();

	let textareaEl: HTMLDivElement | undefined = $state();
</script>

<Textarea
	bind:textBoxEl={textareaEl}
	{autofocus}
	class="text-12 text-body {className}"
	{value}
	{disabled}
	{oninput}
	flex="1"
	fontSize={12}
	placeholder="Series description"
	unstyled
	padding={{ top: 2, right: 4, bottom: 2, left: 4 }}
	onkeydown={(e: KeyboardEvent & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
		if (e.key === 'Escape') {
			textareaEl?.blur();

			if (value === '') {
				onEmpty?.();
			}
		}
	}}
/>
