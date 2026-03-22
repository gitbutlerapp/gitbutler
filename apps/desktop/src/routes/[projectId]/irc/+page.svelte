<script lang="ts">
	import { page } from "$app/state";
	import IrcChat from "$components/irc/IrcChat.svelte";

	const projectId = $derived(page.params.projectId!);
</script>

<IrcChat {projectId} />
