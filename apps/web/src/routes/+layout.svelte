<script lang="ts">
	import { goto } from '$app/navigation';
	import { beforeNavigate } from '$app/navigation';
	import { page } from '$app/state';
	// import Header from '$home/components/Header.svelte';
	import HomePage from '$home/HomePage.svelte';
	import { AuthService, AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import * as jsonLinks from '$lib/data/links.json';
	import { latestClientVersion } from '$lib/store';
	import { getValidReleases } from '$lib/types/releases';
	import { UserService, USER_SERVICE } from '$lib/user/userService';
	import { updateFavIcon } from '$lib/utils/faviconUtils';
	import { provide } from '@gitbutler/core/context';
	import { HttpClient, HTTP_CLIENT } from '@gitbutler/shared/network/httpClient';
	import { WebRoutesService, WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { type Snippet } from 'svelte';
	import { env } from '$env/dynamic/public';
	import '../styles/global.css';
	import '@gitbutler/design-core/tokens';
	import '@gitbutler/design-core/fonts';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const routesService = new WebRoutesService(location.protocol + '//' + location.host, true);
	provide(WEB_ROUTES_SERVICE, routesService);

	const authService = new AuthService();
	provide(AUTH_SERVICE, authService);

	const httpClient = new HttpClient(window.fetch, env.PUBLIC_APP_HOST, authService.tokenReadable);
	provide(HTTP_CLIENT, httpClient);

	const userService = new UserService(httpClient);
	provide(USER_SERVICE, userService);

	const persistedToken = authService.token;

	// Releases data for changelog
	let releases: any[] = $state([]);

	// Check if current page should use marketing layout
	const isMarketingPage = $derived(
		(page.route.id === '/(app)' && !persistedToken.current) ||
			page.route.id === '/(app)/home' ||
			page.route.id === '/downloads' ||
			page.route.id === '/nightlies'
	);

	// Check if current page should render children directly
	const shouldRenderChildren = $derived(
		page.route.id === '/downloads' || page.route.id === '/nightlies'
	);

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

	// Fetch latest version and releases when showing marketing page
	$effect(() => {
		if (isMarketingPage) {
			// Fetch latest version
			fetch('https://app.gitbutler.com/api/downloads?limit=1&channel=release')
				.then((response) => response.json())
				.then((data) => {
					const latestReleases = getValidReleases(data);
					if (latestReleases.length > 0) {
						latestClientVersion.set(latestReleases[0].version);
					}
				})
				.catch((error) => {
					console.error('Failed to fetch latest version:', error);
				});

			// Fetch latest 10 releases for changelog
			fetch('https://app.gitbutler.com/api/downloads?limit=10&channel=release')
				.then((response) => response.json())
				.then((data) => {
					releases = getValidReleases(data);
				})
				.catch((error) => {
					console.error('Failed to fetch releases for changelog:', error);
				});
		}
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

{#if isMarketingPage}
	<section class="marketing-page">
		{#if shouldRenderChildren}
			{@render children?.()}
		{:else}
			<HomePage {releases} />
		{/if}
	</section>
{:else}
	{@render children?.()}
{/if}

<style>
	.marketing-page {
		display: grid;
		grid-template-columns:
			[full-start]
			1fr 1fr
			[narrow-start]
			1fr 1fr 1fr 1fr 1fr 1fr 1fr 1fr
			[narrow-end]
			1fr [off-gridded] 1fr
			[full-end];
		column-gap: var(--layout-col-gap);
		row-gap: 60px;
		align-items: start;
		width: 100%;
		max-width: 1440px;
		min-height: 100dvh;
		margin: 0 auto;
		padding: 0 var(--layout-side-paddings);

		@media (--desktop-small-viewport) {
			grid-template-columns:
				[full-start]
				1fr
				[narrow-start]
				1fr 1fr 1fr 1fr 1fr 1fr 1fr 1fr 1fr 1fr
				[narrow-end off-gridded]
				1fr
				[full-end];
		}

		@media (--mobile-viewport) {
			grid-template-columns:
				[full-start narrow-start]
				1fr 1fr 1fr 1fr
				[narrow-end full-end off-gridded];
			row-gap: 40px;
		}
	}
</style>
