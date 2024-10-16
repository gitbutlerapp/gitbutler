<script lang="ts">
	import BorderlessTextarea, {
		type Props as BorderlessTextareaProps
	} from '$lib/BorderlessTextarea.svelte';
	import Button from '$lib/Button.svelte';

	const props: BorderlessTextareaProps = $props();
	let value = $state('Hello world');

	function fillTheForm() {
		value =
			'If this is a WIP PR and you have todos left, feel free to uncomment this and turn this PR into a draft, see https://github.blog/2019-02-14-introducing-draft-pull-requests/';
	}
</script>

<div class="wrapper">
	<BorderlessTextarea
		bind:value
		oninput={(e) => {
			console.log(e.currentTarget.value);
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
