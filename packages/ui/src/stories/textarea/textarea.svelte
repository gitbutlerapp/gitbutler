<script lang="ts">
	import Button from '$lib/Button.svelte';
	import Textarea, { type Props as TextareaProps } from '$lib/Textarea.svelte';

	const props: TextareaProps = $props();

	let changableValue = $state('');

	function fillTheForm() {
		changableValue =
			'If this is a WIP PR and you have todos left, feel free to uncomment this and turn this PR into a draft, see https://github.blog/2019-02-14-introducing-draft-pull-requests/';
	}
</script>

<div class="wrapper">
	<Textarea
		value={!changableValue ? props.value : changableValue}
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
			console.log(e.currentTarget.value);
		}}
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
