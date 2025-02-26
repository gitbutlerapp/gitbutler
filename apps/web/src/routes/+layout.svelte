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
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { setContext, type Snippet } from 'svelte';
	import { get } from 'svelte/store';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import '$lib/styles/global.css';
	import '$home/styles/styles.css';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const routesService = new WebRoutesService(location.protocol + '//' + location.host, true);
	setContext(WebRoutesService, routesService);

	const authService = new AuthService();
	setContext(AuthService, authService);

	let token = $state<string | null>();

	const publicRouteIds = ['/(app)/downloads'];
	const isPublicRoute = $derived(publicRouteIds.includes(page.route.id));

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
		if (page.route.id === '/privacy') {
			window.location = jsonLinks.legal.privacyPolicy.url;
		}

		if (!token && !isPublicRoute) {
			goto('/');
		}
	});
</script>

{#if isPublicRoute || token}
	{@render children?.()}
{:else if !token || page.route.id === '/(app)/home'}
	<section class="page-wrapper">
		<Header />
		<Hero />
		<Features />
		<DevelopersReview />
		<BlogHighlights />
		<FAQ />
		<HomeFooter />
	</section>
{/if}
