<script lang="ts">
	import IrcChat from "$components/irc/IrcChat.svelte";
	import type { PageData } from "./$types";

	const { data }: { data: PageData } = $props();
	const projectId = $derived(data.projectId);
</script>

<IrcChat {projectId} />
