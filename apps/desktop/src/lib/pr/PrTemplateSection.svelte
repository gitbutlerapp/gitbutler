<script lang="ts">
	import { ForgeService } from '$lib/backend/forge';
	import { Project } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { getContext } from '@gitbutler/shared/context';

	interface Props {
		pullRequestTemplateBody: string | undefined;
	}

	let { pullRequestTemplateBody = $bindable() }: Props = $props();

	const project = getContext(Project);
	const forgeService = getContext(ForgeService);

	let allAvailableTemplates = $state<{ label: string; value: string }[]>([]);
	const multipleTemplatesAvailable = $derived(allAvailableTemplates.length > 1);

	const defaultReviewTemplatePath = $derived(
		project.git_host.reviewTemplatePath ?? allAvailableTemplates[0]?.value
	);
	let selectedReviewTemplatePath = $state<string | undefined>(undefined);
	const actualReviewTemplatePath = $derived(
		selectedReviewTemplatePath ?? defaultReviewTemplatePath
	);

	console.log('defaultReviewTemplatePath', defaultReviewTemplatePath);

	let useReviewTemplate = $state<boolean | undefined>(undefined);
	const defaultUseReviewTemplate = $derived(!!project.git_host.reviewTemplatePath);
	const actualUseReviewTemplate = $derived(useReviewTemplate ?? defaultUseReviewTemplate);

	function handleToggleUseTemplate() {
		const value: boolean = !actualUseReviewTemplate;

		updateTemplate: {
			if (!value) {
				selectedReviewTemplatePath = undefined;
				pullRequestTemplateBody = undefined;
				break updateTemplate;
			}
			selectedReviewTemplatePath = defaultReviewTemplatePath;
		}

		useReviewTemplate = value;
	}

	// Fetch PR template content
	$effect(() => {
		console.log('defaultReviewTemplatePath', defaultReviewTemplatePath);

		if (actualUseReviewTemplate && actualReviewTemplatePath) {
			forgeService.getReviewTemplateContent(actualReviewTemplatePath).then((template) => {
				pullRequestTemplateBody = template;
			});
		}
	});

	// Fetch available PR templates
	$effect(() => {
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
</script>

<!-- SELECT OR DISPLAY THE AVAILABLE TEMPLATES -->
<!-- {#snippet templatePath()}
	{#if multipleTemplatesAvailable}
		<div class="pr-header__row">
			<Select
				value={actualReviewTemplatePath}
				options={allAvailableTemplates.map(({ label, value }) => ({ label, value }))}
				wide={true}
				searchable
				disabled={allAvailableTemplates.length === 0}
				onselect={(value) => {
					selectedReviewTemplatePath = value;
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === actualReviewTemplatePath} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		</div>
	{/if}
{/snippet} -->

<div class="pr-template__wrap">
	<!-- <label class="pr-toggle" for="use-template">
		<span class="text-12 pr-template__toggle__label">Use PR template</span>
		<Toggle
			id="use-template"
			small
			checked={actualUseReviewTemplate}
			on:click={handleToggleUseTemplate}
		/>
	</label> -->

	<Select
		value={actualReviewTemplatePath}
		options={allAvailableTemplates.map(({ label, value }) => ({ label, value }))}
		placeholder="No PR templates found ¯\_(ツ)_/¯"
		flex="1"
		searchable
		disabled={allAvailableTemplates.length <= 1}
		onselect={(value) => {
			selectedReviewTemplatePath = value;
		}}
	>
		{#snippet itemSnippet({ item, highlighted })}
			<SelectItem selected={item.value === actualReviewTemplatePath} {highlighted}>
				{item.label}
			</SelectItem>
		{/snippet}
	</Select>
</div>

<style lang="postcss">
	.pr-template__wrap {
		display: flex;
		gap: 6px;
	}
</style>
