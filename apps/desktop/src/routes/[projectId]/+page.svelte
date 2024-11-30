<script lang="ts">
	import { run } from 'svelte/legacy';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	const projectId = $derived($page.params.projectId);
	run(() => {
		if (projectId) {
			goto(`/${projectId}/board`, { replaceState: true });
		}
	});
</script>
