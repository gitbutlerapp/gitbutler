<script lang="ts">
	import FullscreenIllustrationCard from '$lib/components/service/FullscreenIllustrationCard.svelte';
</script>

<svelte:head>
	<title>GitButler | Logged in</title>
</svelte:head>

<FullscreenIllustrationCard>
	{#snippet title()}
		You are succesfully logged in!
	{/snippet}

	<div>You can now close this window and return to your client.</div>
</FullscreenIllustrationCard>
