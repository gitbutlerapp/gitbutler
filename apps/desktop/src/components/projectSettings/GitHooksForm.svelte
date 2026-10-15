<script lang="ts">
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import SettingsSection from "$components/shared/SettingsSection.svelte";
	import { projectRunCommitHooks } from "$lib/config/config";
	import { t } from "$lib/i18n";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { inject } from "@gitbutler/core/context";
	import { CardGroup, Toggle } from "@gitbutler/ui-svelte";
	import type { Project } from "$lib/project/project";

	const { projectId }: { projectId: string } = $props();
	const runCommitHooks = $derived(projectRunCommitHooks(projectId));
	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));

	async function onHuskyHooksEnabledClick(project: Project, value: boolean) {
		await projectsService.updateProject({ ...project, husky_hooks_enabled: value });
	}
</script>

<SettingsSection>
	<CardGroup>
		<CardGroup.Item labelFor="runHooks">
			{#snippet title()}
				{t("run-git-hooks")}
			{/snippet}
			{#snippet caption()}
				{t("enable-running-git-hooks-pre-push-pre-post-commit-commit-msg")}
			{/snippet}
			{#snippet actions()}
				<Toggle id="runHooks" bind:checked={$runCommitHooks} />
			{/snippet}
		</CardGroup.Item>
	</CardGroup>

	<ReduxResult {projectId} result={projectQuery.result}>
		{#snippet children(project)}
			<CardGroup>
				<CardGroup.Item labelFor="huskyHooks">
					{#snippet title()}
						{t("enable-husky-hooks")}
					{/snippet}
					{#snippet caption()}
						{t("only-enable-this-for-repositories-you-trust")}
						<br />
						{t("allow-gitbutler-to-execute-scripts-from-husky-which-can-come")}
					{/snippet}
					{#snippet actions()}
						<Toggle
							id="huskyHooks"
							checked={project.husky_hooks_enabled}
							onchange={(checked) => onHuskyHooksEnabledClick(project, checked)}
						/>
					{/snippet}
				</CardGroup.Item>
			</CardGroup>
		{/snippet}
	</ReduxResult>
</SettingsSection>
