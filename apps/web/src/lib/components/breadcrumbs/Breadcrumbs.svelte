<script lang="ts">
	import { getBreadcrumbsContext } from '$lib/components/breadcrumbs/breadcrumbsContext.svelte';

	const breadcrumbsContext = getBreadcrumbsContext();

	$inspect(breadcrumbsContext);
</script>
