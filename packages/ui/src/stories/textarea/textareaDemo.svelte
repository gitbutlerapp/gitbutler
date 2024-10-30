<script lang="ts">
	import Button from '$lib/Button.svelte';
	import Textarea, { type Props as TextareaProps } from '$lib/Textarea.svelte';

	const props: TextareaProps = $props();

	let changableValue = $state(props.value);

	function fillTheForm() {
		changableValue = `## ☕️ Reasoning


## 🧢 Changesd


## 📌 Todos`;
	}

	function handleDescriptionKeyDown(e: KeyboardEvent & { currentTarget: HTMLTextAreaElement }) {
		if (e.key === 'Escape') {
			console.log('keyboard', e.key);
			e.preventDefault();
			return;
		}

		if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
			console.log('keyboard', e.key);
			e.preventDefault();
			return;
		}
	}
</script>

<div class="wrapper">
	<Textarea
		label={props.label}
		value={changableValue}
		placeholder={props.placeholder}
		minRows={props.minRows}
		maxRows={props.maxRows}
		autofocus={props.autofocus}
		disabled={props.disabled}
		borderBottom={props.borderBottom}
		borderLeft={props.borderLeft}
		borderRight={props.borderRight}
		borderTop={props.borderTop}
		borderless={props.borderless}
		unstyled={props.unstyled}
		oninput={(e) => {
			console.log('input', e);
		}}
		onkeydown={handleDescriptionKeyDown}
		onfocus={(e) => {
			console.log('focus', e);
		}}
	/>
	<Button style="ghost" outline onclick={fillTheForm}>Fill the form</Button>
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		max-width: 300px;
		gap: 12px;
	}
</style>
