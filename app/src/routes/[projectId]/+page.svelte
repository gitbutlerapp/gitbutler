<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	// eslint-disable-next-line svelte/valid-compile
	$: projectId = $page.params.projectId;
	$: if (projectId) {
		goto(`/${projectId}/board`, { replaceState: true });
	}
</script>
