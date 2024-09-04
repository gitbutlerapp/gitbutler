<script lang="ts">
	import InfoButton from '$lib/InfoButton.svelte';

	const props = $props();
</script>

<div class="wrapper">
	<div class="message-1">
		<p class="text-13 text">hello world! Here is some more info for you</p>
		<InfoButton>
			<div class="info-tooltip">
				<h2 class="text-18 text">
					{props.headerContent}
				</h2>
				<p class="text-13 text">
					{props.bodyContent + ' '}<a class="link" href={props.linkAddress}>{props.linkText}</a>.
				</p>
			</div>
		</InfoButton>
	</div>
	<div class="message-2">
		<p class="text-13 text">
			This is an example of only text <InfoButton
				><a class="link" href={props.linkAddress}>{props.linkText}</a></InfoButton
			>
		</p>
	</div>
</div>

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 20px;
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

	.info-tooltip {
		padding: 4px 8px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.text {
		color: var(--clr-text-1);
	}

	.link {
		color: var(--clr-text-1);
		text-decoration: underline;
	}
</style>
