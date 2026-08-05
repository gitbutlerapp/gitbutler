<script lang="ts">
	import AgentSkillList from "$components/settings/AgentSkillList.svelte";
	import CliInstallCard from "$components/settings/CliInstallCard.svelte";
	import { Spacer } from "@gitbutler/ui";
	import type { SkillScope } from "@gitbutler/but-sdk";

	type Props = {
		projectId?: string;
	};

	const { projectId }: Props = $props();

	// One component serves both settings modals. The only real difference is
	// scope, which mirrors the same distinction the backend models, so this is
	// not an invented abstraction.
	const scope = $derived<SkillScope>(projectId ? "repository" : "global");
</script>

{#if !projectId}
	<CliInstallCard />
	<Spacer />
{/if}

<h3 class="text-14 text-semibold m-b-4">
	{projectId ? "Skills for this project" : "Skills for all your projects"}
</h3>
<p class="text-12 text-body clr-text-2 m-b-12">
	{projectId
		? "Installed into this repository, so they travel with it and apply to everyone who works here."
		: "Installed into your home directory, so they apply everywhere you use this agent."}
</p>

<AgentSkillList {scope} {projectId} />
