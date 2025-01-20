<script lang="ts">
	import { v3 } from '$lib/config/uiFeatureFlags';
	import type { LayoutData } from '../../$types';
	import { goto } from '$app/navigation';

	const { data }: { data: LayoutData } = $props();

	// Redirect to board if we have switched away from V3 feature.
	$effect(() => {
		if (!$v3) {
			goto(`/${data.projectId}/board`);
		}
	});
</script>

<div>Workspace Page</div>
