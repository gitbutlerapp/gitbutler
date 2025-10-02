<script lang="ts">
	import GitbutlerLogoLink from '$lib/components/GitbutlerLogoLink.svelte';
	import HeaderAuthSection from '$lib/components/HeaderAuthSection.svelte';

	interface NavigationProps {
		markOnly?: boolean;
	}

	const { markOnly }: NavigationProps = $props();
</script>

<nav class="navigation" class:justify-center={markOnly} class:justify-between={!markOnly}>
	{#if markOnly}
		<GitbutlerLogoLink markOnly />
	{:else}
		<GitbutlerLogoLink markOnly />
		<HeaderAuthSection />
	{/if}
</nav>

<style lang="postcss">
	.navigation {
		display: flex;
		align-items: center;
		width: 100%;
		padding-bottom: 24px;
		gap: 16px;
	}
</style>
