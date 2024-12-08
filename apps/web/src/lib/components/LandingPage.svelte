<script lang="ts">
	import Header from './Header.svelte';
	import * as jsonLinks from '$lib/data/links.json';
	import BlogHighlights from '$lib/sections/BlogHighlights.svelte';
	import DevelopersReview from '$lib/sections/DevelopersReview.svelte';
	import FAQ from '$lib/sections/FAQ.svelte';
	import Features from '$lib/sections/Features.svelte';
	import Footer from '$lib/sections/Footer.svelte';
	import Hero from '$lib/sections/Hero.svelte';
	import { targetDownload } from '$lib/store';
	import { latestClientVersion } from '$lib/store';
	import { getOS } from '$lib/utils/getOS';
	import GhostContentAPI from '@tryghost/content-api';
	import { onMount } from 'svelte';

	const GHOST_URL = 'https://gitbutler.ghost.io';
	const GHOST_KEY = '80bbdca8b933f3d98780c7cc1b';
	const GHOST_VERSION = 'v5.0';

	let data: any;
	onMount(async () => {
		const { postsJson } = await loadBlog();
		data = { postsJson };
	});

	export async function loadBlog() {
		const api = GhostContentAPI({
			url: GHOST_URL,
			key: GHOST_KEY,
			version: GHOST_VERSION
		});
		const postsJson = await api.posts.browse({ limit: 3, include: 'authors' });
		return { postsJson };
	}

	import '../../styles/styles.css';

	onMount(() => {
		const os = getOS();

		if (os === 'macos') {
			targetDownload.set(jsonLinks.downloads.appleSilicon);
		} else if (os === 'linux') {
			targetDownload.set(jsonLinks.downloads.linuxDeb);
		} else if (os === 'windows') {
			targetDownload.set(jsonLinks.downloads.windowsMsi);
		} else {
			targetDownload.set(jsonLinks.downloads.appleSilicon);
		}

		// get actual latest version from https://app.gitbutler.com/latest_version
		fetch('https://app.gitbutler.com/latest_version')
			.then((res) => res.text())
			.then((data) => {
				latestClientVersion.set(data);
			});
	});
</script>

<Header />
<Hero />
<Features />
<DevelopersReview />
{#if data}
	<BlogHighlights posts={data.postsJson} />
{/if}
<FAQ />
<Footer />
