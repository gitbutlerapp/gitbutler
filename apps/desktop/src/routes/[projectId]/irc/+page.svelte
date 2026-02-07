<script lang="ts">
	import IrcChat from "$components/IrcChat.svelte";
	import { page } from "$app/state";

	const projectId = $derived(page.params.projectId!);
</script>

<IrcChat {projectId} />
