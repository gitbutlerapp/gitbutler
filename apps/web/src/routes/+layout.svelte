<script lang="ts">
	import { goto } from '$app/navigation';
	import { beforeNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import Header from '$home/components/Header.svelte';
	import * as jsonLinks from '$home/data/links.json';
	import BlogHighlights from '$home/sections/BlogHighlights.svelte';
	import DevelopersReview from '$home/sections/DevelopersReview.svelte';
	import FAQ from '$home/sections/FAQ.svelte';
	import Features from '$home/sections/Features.svelte';
	import HomeFooter from '$home/sections/Footer.svelte';
	import Hero from '$home/sections/Hero.svelte';
	import { AuthService, AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import { updateFavIcon } from '$lib/utils/faviconUtils';
	import { provide } from '@gitbutler/core/context';
	import { WebRoutesService, WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { type Snippet } from 'svelte';
	import '../styles/global.css';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const routesService = new WebRoutesService(location.protocol + '//' + location.host, true);
	provide(WEB_ROUTES_SERVICE, routesService);

	const authService = new AuthService();
	provide(AUTH_SERVICE, authService);

	const persistedToken = authService.token;

	$effect(() => {
		if (page.url.searchParams.has('gb_access_token')) {
			const token = page.url.searchParams.get('gb_access_token');
			if (token && token !== persistedToken.current) {
				authService.setToken(token);
			}

			page.url.searchParams.delete('gb_access_token');
			goto(`?${page.url.searchParams.toString()}`);
		}
	});

	$effect(() => {
		if (page.url.pathname === '/privacy') {
			window.location.href = jsonLinks.legal.privacyPolicy.url;
		}

		if (!persistedToken.current && page.route.id === '/(app)/home') {
			goto('/');
		}
	});

	beforeNavigate(() => {
		updateFavIcon(); // reset the icon
	});
</script>

<svelte:head>
	{#if import.meta.env.MODE !== 'development'}
		<script
			async
			src="https://u.gitbutler.com/script.js"
			data-website-id="c406f339-a2af-4992-9a82-162134323008"
		></script>
	{/if}
</svelte:head>

{#if (page.route.id === '/(app)' && !persistedToken.current) || page.route.id === '/(app)/home'}
	<section class="marketing-page">
		<Header />
		<Hero />
		<Features />
		<DevelopersReview />
		<BlogHighlights />
		<FAQ />
		<HomeFooter />
	</section>
{:else}
	{@render children?.()}
{/if}

<style>
	.marketing-page {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 1440px;
		margin: 0 auto;
		padding: 0 60px;

		font-family: var(--fontfamily-mono);

		/* optimise font rendering */
		-webkit-font-smoothing: antialiased;
		color: var(--clr-black);
		text-rendering: optimizeLegibility;

		-webkit-font-smoothing: antialiased;
		-moz-osx-font-smoothing: grayscale;
		text-rendering: optimizeLegibility;

		@media (--mobile-viewport) {
			padding: 0 20px;
		}

		@media (--desktop-small-viewport) {
			padding: 0 40px;
		}

		@media (--desktop-viewport) {
			overflow-x: hidden;
		}
	}
</style>
