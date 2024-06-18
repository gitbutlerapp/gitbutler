<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	$: projectId = $page.params.projectId;
	$: if (projectId) {
		goto(`/${projectId}/board`, { replaceState: true });
	}
</script>
