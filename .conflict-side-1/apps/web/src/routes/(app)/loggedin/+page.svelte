<script lang="ts">
	import FullscreenUtilityCard from '$lib/components/service/FullscreenUtilityCard.svelte';
</script>

<svelte:head>
	<title>GitButler | Logged in</title>
</svelte:head>

<FullscreenUtilityCard
	title="Signed in successfully 🎯"
	backlink={{
		label: 'Main',
		href: '/'
	}}
>
	<p class="text-14 clr-text-2">You can now close this window and return to your client.</p>
</FullscreenUtilityCard>
