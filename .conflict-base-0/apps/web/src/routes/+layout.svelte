<script lang="ts">
	import Header from '$home/components/Header.svelte';
	import * as jsonLinks from '$home/data/links.json';
	import BlogHighlights from '$home/sections/BlogHighlights.svelte';
	import DevelopersReview from '$home/sections/DevelopersReview.svelte';
	import FAQ from '$home/sections/FAQ.svelte';
	import Features from '$home/sections/Features.svelte';
	import HomeFooter from '$home/sections/Footer.svelte';
	import Hero from '$home/sections/Hero.svelte';
	import { AuthService } from '$lib/auth/authService.svelte';
	import { updateFavIcon } from '$lib/utils/faviconUtils';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { setContext, type Snippet } from 'svelte';
	import { get } from 'svelte/store';
	import { beforeNavigate } from '$app/navigation';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import '$lib/styles/global.css';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const routesService = new WebRoutesService(location.protocol + '//' + location.host, true);
	setContext(WebRoutesService, routesService);

	const authService = new AuthService();
	setContext(AuthService, authService);

	let token = $state<string | null>();

	$effect(() => {
		token = get(authService.tokenReadable) || page.url.searchParams.get('gb_access_token');
		if (token) {
			authService.setToken(token);

			if (page.url.searchParams.has('gb_access_token')) {
				page.url.searchParams.delete('gb_access_token');
				goto(`?${page.url.searchParams.toString()}`);
			}
		}
	});

	$effect(() => {
		if (page.url.pathname === '/privacy') {
			window.location.href = jsonLinks.legal.privacyPolicy.url;
		}

		if (!token && page.route.id === '/(app)/home') {
			goto('/');
		}
	});

	beforeNavigate(() => {
		updateFavIcon(); // reset the icon
	});
</script>

{#if (page.route.id === '/(app)' && !token) || page.route.id === '/(app)/home'}
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
		max-width: 1440px;
		width: 100%;
		margin: 0 auto;
		padding: 0 60px;

		font-family: 'Spline Sans Mono', monospace;

		/* optimise font rendering */
		-webkit-font-smoothing: antialiased;
		text-rendering: optimizeLegibility;
		color: var(--clr-black);

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
