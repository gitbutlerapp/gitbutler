<script lang="ts">
	import { ForgeService } from '$lib/backend/forge';
	import { ProjectService, ProjectsService } from '$lib/backend/projects';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import { getContext } from '@gitbutler/shared/context';

	interface Props {
		pullRequestTemplateBody: string | undefined;
	}

	let { pullRequestTemplateBody = $bindable() }: Props = $props();

	const projectsService = getContext(ProjectsService);
	const projectService = getContext(ProjectService);
	const forgeService = getContext(ForgeService);

	let allAvailableTemplates = $state<{ label: string; value: string }[]>([]);

	const projectStore = projectService.project;
	const project = $derived($projectStore);
	const reviewTemplatePath = $derived(project?.git_host.reviewTemplatePath);
	const show = $derived(!!reviewTemplatePath);

	// Fetch PR template content
	$effect(() => {
		if (!project) return;
		if (reviewTemplatePath) {
			forgeService.getReviewTemplateContent(reviewTemplatePath).then((template) => {
				pullRequestTemplateBody = template;
			});
		}
	});

	// Fetch available PR templates
	$effect(() => {
		if (!project) return;
		forgeService.getAvailableReviewTemplates().then((availableTemplates) => {
			if (availableTemplates) {
				allAvailableTemplates = availableTemplates.map((availableTemplate) => {
					return {
						label: availableTemplate,
						value: availableTemplate
					};
				});
			}
		});
	});

	async function setPullRequestTemplatePath(value: string) {
		if (!project) return;
		project.git_host.reviewTemplatePath = value;
		await projectsService.updateProject(project);
	}

	export async function setUsePullRequestTemplate(value: boolean) {
		if (!project) return;

		setTemplate: {
			if (!value) {
				project.git_host.reviewTemplatePath = undefined;
				pullRequestTemplateBody = undefined;
				break setTemplate;
			}

			if (allAvailableTemplates[0]) {
				project.git_host.reviewTemplatePath = allAvailableTemplates[0].value;
				break setTemplate;
			}
		}

		await projectsService.updateProject(project);
	}

	export const imports = {
		get showing() {
			return show;
		},
		get hasTemplates() {
			return allAvailableTemplates.length > 0;
		}
	};
</script>

{#if show}
	<div class="pr-template__wrap">
		<Select
			value={reviewTemplatePath}
			options={allAvailableTemplates.map(({ label, value }) => ({ label, value }))}
			placeholder="No PR templates found ¯\_(ツ)_/¯"
			flex="1"
			searchable
			disabled={allAvailableTemplates.length <= 1}
			onselect={setPullRequestTemplatePath}
		>
			{#snippet itemSnippet({ item, highlighted })}
				<SelectItem selected={item.value === reviewTemplatePath} {highlighted}>
					{item.label}
				</SelectItem>
			{/snippet}
		</Select>
	</div>
{/if}

<style lang="postcss">
	.pr-template__wrap {
		display: flex;
		gap: 6px;
	}
</style>
