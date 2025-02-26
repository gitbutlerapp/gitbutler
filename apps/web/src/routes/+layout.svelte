<script lang="ts">
	import Header from '$home/components/Header.svelte';
	import BlogHighlights from '$home/sections/BlogHighlights.svelte';
	import DevelopersReview from '$home/sections/DevelopersReview.svelte';
	import FAQ from '$home/sections/FAQ.svelte';
	import Features from '$home/sections/Features.svelte';
	import HomeFooter from '$home/sections/Footer.svelte';
	import Hero from '$home/sections/Hero.svelte';
	import { AuthService } from '$lib/auth/authService.svelte';
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

	const authService = new AuthService();
	setContext(AuthService, authService);
	let token = $state<string | null>();

	// Parse searchParams for token
	const searchParams = $derived(page.url.searchParams);

	$effect(() => {
		token = get(authService.tokenReadable) || searchParams.get('gb_access_token');
		if (token) {
			authService.setToken(token);

			if (page.url.searchParams.has('gb_access_token')) {
				page.url.searchParams.delete('gb_access_token');
				goto(`?${page.url.searchParams.toString()}`);
			}
		}
	});

	$effect(() => {
		$inspect('rootLayout.token', token);
		if (!token) {
			goto('/');
		}
	});
</script>

{#if !token || page.route.id === '/(app)/home'}
	<section class="page-wrapper">
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
