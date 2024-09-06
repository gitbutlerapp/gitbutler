<script lang="ts">
	import InfoButton from '$lib/InfoButton.svelte';

	const props = $props();
</script>

<div class="wrapper">
	<div class="message-1">
		<p class="text-13 text">
			This is an example with the title property <InfoButton title={props.title} size={props.size}>
				This is an example of text with a link inside an InfoButton. Here is a <a
					href="gitbutler.com"
					target="_blank"
					class="link">link to gitbutler</a
				>
			</InfoButton>
		</p>
	</div>

	<div class="message-2">
		<p class="text-13 text">
			This is an example of only text <InfoButton size={props.size}
				>Text only example. Link to <a href="gitbutler.com" target="_blank" class="link"
					>gitbutler</a
				>
			</InfoButton>
		</p>
	</div>
</div>

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.message-1 {
		display: flex;
		gap: 4px;
	}

	.message-2 {
		display: flex;
		align-items: start;
		gap: 4px;
	}

	.text {
		color: var(--clr-text-1);
	}

	.link {
		text-decoration: underline;
	}
</style>
