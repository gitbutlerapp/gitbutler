<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId);

	$effect(() => {
		if (projectId) {
			goto(`/${projectId}/workspace`, { replaceState: true });
		}
	});
</script>
