<script lang="ts">
	import { getBreadcrumbsContext } from '$lib/components/breadcrumbs/breadcrumbsContext.svelte';
	import type { OwnerParameters } from '@gitbutler/shared/routing/webRoutes';

	interface Props {
		data: OwnerParameters;
	}

	let { data }: Props = $props();

	const breadcrumbsContext = getBreadcrumbsContext();
	$effect(() => {
		breadcrumbsContext.current = data;
	});
</script>

<h2>Owner: {data.ownerSlug}</h2>
