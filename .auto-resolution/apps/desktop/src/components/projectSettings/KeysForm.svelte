<script lang="ts">
	import CredentialCheck from "$components/projectSettings/CredentialCheck.svelte";
	import ProjectNameLabel from "$components/shared/ProjectNameLabel.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import SettingsSection from "$components/shared/SettingsSection.svelte";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { inject } from "@gitbutler/core/context";

	interface Props {
		// Used by credential checker before target branch set
		projectId: string;
		remoteName?: string;
		branchName?: string;
		showProjectName?: boolean;
		disabled?: boolean;
	}

	const {
		projectId,
		remoteName = "",
		branchName = "",
		showProjectName = false,
		disabled = false,
	}: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchQuery.response);
	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));
</script>

<div>
	<ReduxResult {projectId} result={projectQuery.result}>
		{#snippet children(project)}
			<SettingsSection>
				{#snippet top()}
					{#if showProjectName}<ProjectNameLabel projectName={project.title} />{/if}
				{/snippet}
				{#snippet title()}
					Git authentication
				{/snippet}
				{#snippet description()}
					GitButler authenticates with your Git remote provider through the Git executable available
					on your PATH.
				{/snippet}
				<CredentialCheck
					{disabled}
					projectId={project.id}
					remoteName={remoteName || baseBranch?.remoteName}
					branchName={branchName || baseBranch?.shortName}
				/>
			</SettingsSection>
		{/snippet}
	</ReduxResult>
</div>
