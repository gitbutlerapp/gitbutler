<script lang="ts">
	import CommitSigningForm from "$components/projectSettings/CommitSigningForm.svelte";
	import GitHooksForm from "$components/projectSettings/GitHooksForm.svelte";
	import KeysForm from "$components/projectSettings/KeysForm.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import SettingsSection from "$components/shared/SettingsSection.svelte";
	import { BACKEND } from "$lib/backend";
	import { projectLandDirectly } from "$lib/config/config";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { inject } from "@gitbutler/core/context";
	import { CardGroup, Spacer, Toggle } from "@gitbutler/ui";
	import type { Project } from "$lib/project/project";

	const { projectId }: { projectId: string } = $props();
	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));
	const backend = inject(BACKEND);
	const landDirectly = $derived(projectLandDirectly(projectId));

	async function onForcePushProtectionClick(project: Project, value: boolean) {
		await projectsService.updateProject({ ...project, force_push_protection: value });
	}
</script>

<SettingsSection>
	<CardGroup>
		<CardGroup.Item labelFor="landDirectly">
			{#snippet title()}
				Land branches directly
			{/snippet}
			{#snippet caption()}
				Replace the "Create PR" button with a "Land" button that integrates the branch straight into
				the target branch, without opening a pull request. Shown on the bottom branch of a stack;
				works without a forge integration.
			{/snippet}
			{#snippet actions()}
				<Toggle id="landDirectly" bind:checked={$landDirectly} />
			{/snippet}
		</CardGroup.Item>
	</CardGroup>

	<GitHooksForm {projectId} />
	<CommitSigningForm {projectId} />
	{#if backend.platformName !== "windows"}
		<Spacer />
		<KeysForm {projectId} showProjectName={false} />
	{/if}

	<Spacer />
	<ReduxResult {projectId} result={projectQuery.result}>
		{#snippet children(project)}
			<CardGroup>
				<CardGroup.Item labelFor="forcePushProtection">
					{#snippet title()}
						Force push protection
					{/snippet}
					{#snippet caption()}
						Protect remote commits during force pushes. This will use Git's safer force push flags
						to avoid overwriting remote commit history.
					{/snippet}
					{#snippet actions()}
						<Toggle
							id="forcePushProtection"
							checked={project.force_push_protection}
							onchange={(checked) => onForcePushProtectionClick(project, checked)}
						/>
					{/snippet}
				</CardGroup.Item>
			</CardGroup>
		{/snippet}
	</ReduxResult>
</SettingsSection>
