<script>
	import LandingPage from '$home/LandingPage.svelte';
	import { cleanBreadcrumbs } from '$lib/components/breadcrumbs/breadcrumbsContext.svelte';

	$effect(cleanBreadcrumbs);
</script>

<LandingPage />
