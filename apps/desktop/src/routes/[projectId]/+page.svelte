<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	let projectId = $derived($page.params.projectId);
	$effect(() => {
		if (projectId) {
			goto(`/${projectId}/board`, { replaceState: true });
		}
	});
</script>
