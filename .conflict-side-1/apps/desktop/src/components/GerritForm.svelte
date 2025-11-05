<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/core/context';
	import { SectionCard, Toggle } from '@gitbutler/ui';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const gbConfig = inject(GIT_CONFIG_SERVICE);
	const projectService = inject(PROJECTS_SERVICE);

	const isGerritProject = $derived(projectService.isGerritProject(projectId));
</script>

<div class="stack-v">
	<ReduxResult {projectId} result={isGerritProject.result}>
		{#snippet children(itIsAGerritProject)}
			<SectionCard orientation="row" labelFor="gerritModeToggle">
				{#snippet title()}
					Gerrit Configuration
				{/snippet}

				{#snippet caption()}
					Enable or disable Gerrit mode for this project.
				{/snippet}

				{#snippet actions()}
					<Toggle
						id="gerritModeToggle"
						checked={itIsAGerritProject}
						onclick={() => gbConfig.setGerritMode(projectId, !itIsAGerritProject)}
					/>
				{/snippet}
			</SectionCard>
		{/snippet}
	</ReduxResult>
</div>
