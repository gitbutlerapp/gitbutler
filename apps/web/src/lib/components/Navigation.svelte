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
		<GitbutlerLogoLink />
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

	.logo {
		display: flex;
		align-items: center;
		overflow: hidden;
		gap: 12px;
	}

	.logo-mark {
		display: flex;
		width: 28px;
		height: 100%;
		margin-top: 5px;
	}

	.logo-text {
		font-size: 44px;
		line-height: 1;
		font-family: var(--fontfamily-accent);
	}

	/* MODIFIERS */
	:global(.navigation .hidden-on-desktop) {
		display: none;

		@media (--mobile-viewport) {
			display: block;
		}
	}
</style>
