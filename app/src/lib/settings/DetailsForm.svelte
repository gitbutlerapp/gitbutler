<script lang="ts">
	import { Project, ProjectService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import TextArea from '$lib/shared/TextArea.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import { User } from '$lib/stores/user';
	import { getContext, getContextStore } from '$lib/utils/context';

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
		project.api = api ? { ...api, sync: true } : undefined;
		projectService.updateProject(project);
	}
</script>

<SectionCard>
	<form>
		<fieldset class="fields-wrapper">
			<TextBox label="Path" readonly id="path" value={project?.path} />
			<section class="description-wrapper">
				<TextBox
					label="Project name"
					id="name"
					placeholder="Project name can't be empty"
					bind:value={title}
					required
					on:change={(e) => {
						project.title = e.detail;
						saveProject();
					}}
				/>
				<TextArea
					id="description"
					rows={3}
					placeholder="Project description"
					bind:value={description}
					on:change={() => {
						project.description = description;
						saveProject();
					}}
					maxHeight={300}
				/>
			</section>
		</fieldset>
	</form>
</SectionCard>
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
</style>
