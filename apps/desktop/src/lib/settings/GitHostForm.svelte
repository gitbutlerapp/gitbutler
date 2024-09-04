<script lang="ts">
	import { Project, ProjectService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import {
		gitHostPullRequestTemplatePath,
		gitHostUsePullRequestTemplate
	} from '$lib/config/config';
	import Spacer from '$lib/shared/Spacer.svelte';
	import TextArea from '$lib/shared/TextArea.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { User } from '$lib/stores/user';
	import { getContext, getContextStore } from '$lib/utils/context';

	const usePullRequestTemplate = gitHostUsePullRequestTemplate();
	const pullRequestTemplatePath = gitHostPullRequestTemplatePath();

	const project = getContext(Project);
	const user = getContextStore(User);
	const projectService = getContext(ProjectService);

	let title = project?.title;
	let description = project?.description;

	async function saveProject() {
		const api =
			$user && project.api
				? await projectService.updateCloudProject($user?.access_token, project.api.repository_id, {
						name: project.title,
						description: project.description
					})
				: undefined;
		project.api = api ? { ...api, sync: false, sync_code: undefined } : undefined;
		projectService.updateProject(project);
	}
</script>

<div>
	<SectionCard roundedBottom={false} orientation="row" labelFor="use-pull-request-template">
		<svelte:fragment slot="title">Pull Request Template</svelte:fragment>
		<svelte:fragment slot="caption">
			Use Pull Request template when creating a Pull Requests.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle id="use-pull-request-template" value="false" bind:checked={$usePullRequestTemplate} />
		</svelte:fragment>
	</SectionCard>
	<SectionCard roundedTop={false} orientation="row" labelFor="pull-request-template-path">
		<svelte:fragment slot="title">Pull Request Template Path</svelte:fragment>
		<svelte:fragment slot="caption">
			<div class="pr-path--label">Path to your Pull Request template in your repository.</div>
			<TextBox
				id="pull-request-template-path"
				bind:value={$pullRequestTemplatePath}
				placeholder=".github/pull_request_template.md"
			/>
		</svelte:fragment>
	</SectionCard>
</div>
<Spacer />

<style>
	.fields-wrapper {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.description-wrapper {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.pr-path--label {
		margin-bottom: 0.75rem;
	}
</style>
