<script lang="ts">
	import { ForgeService } from '$lib/backend/forge';
	import { Project } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Segment from '@gitbutler/ui/segmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/segmentControl/SegmentControl.svelte';

	interface Props {
		isDisplay: boolean;
		actualTitle: string;
		isEditing: boolean;
		pullRequestTemplateBody: string | undefined;
	}

	let {
		isDisplay,
		actualTitle,
		isEditing = $bindable(),
		pullRequestTemplateBody = $bindable()
	}: Props = $props();

	const project = getContext(Project);
	const forgeService = getContext(ForgeService);

	let allAvailableTemplates = $state<{ label: string; value: string }[]>([]);
	const multipleTemplatesAvailable = $derived(allAvailableTemplates.length > 1);
	let selectedReviewTemplatePath = $state<string | undefined>(undefined);
	const defaultReviewTemplatePath = $derived(project.git_host.reviewTemplatePath);
	const actualReviewTemplatePath = $derived(
		selectedReviewTemplatePath ?? defaultReviewTemplatePath
	);

	let useReviewTemplate = $state<boolean | undefined>(undefined);
	const defaultUseReviewTemplate = $derived(!!project.git_host.reviewTemplatePath);
	const actualUseReviewTemplate = $derived(useReviewTemplate ?? defaultUseReviewTemplate);

	// Fetch PR template content
	$effect(() => {
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
</script>

<!-- SELECT OR DISPLAY THE AVAILABLE TEMPLATES -->
{#snippet templatePath()}
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
{/snippet}

<!-- MAIN -->
<div class="pr-header">
	{#if isDisplay}
		<div class="pr-header__row">
			<h3 class="text-head-22 text-semibold pr-title">{actualTitle}</h3>
		</div>
	{:else}
		<div class="pr-header__row">
			<h3 class="text-14 text-semibold pr-title" class:text-head-22={!isEditing}>
				{isEditing ? 'Create a pull request' : actualTitle}
			</h3>
			<SegmentControl
				defaultIndex={isDisplay ? 1 : 0}
				onselect={(id) => {
					isEditing = id === 'write';
				}}
			>
				<Segment id="write">Edit</Segment>
				<Segment id="preview">Preview</Segment>
			</SegmentControl>
		</div>

		{#if isEditing}
			<SectionCard orientation="column">
				<div class="pr-header__row">
					<label class="template-toggle__wrap">
						<Toggle
							id="use-template"
							small
							checked={actualUseReviewTemplate}
							on:click={handleToggleUseTemplate}
						/>
						<label class="text-12 template-toggle__label" for="use-template">Use PR template</label>
					</label>
				</div>
				{#if actualUseReviewTemplate}
					{@render templatePath()}
				{/if}
			</SectionCard>
		{/if}
	{/if}
</div>

<style>
	.pr-header {
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 16px;
		padding: 16px 16px 14px;
	}

	.pr-header__row {
		width: 100%;
		display: flex;
		align-items: center;
	}

	.pr-title {
		flex: 1;
		margin-top: 4px;
	}

	.template-toggle__wrap {
		display: flex;
		align-items: center;
		gap: 10px;
	}
</style>
