<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import { getContext } from '@gitbutler/shared/context';

	interface Props {
		pullRequestTemplateBody: string | undefined;
	}

	let { pullRequestTemplateBody = $bindable() }: Props = $props();

	const project = getContext(Project);

	let allAvailableTemplates = $state<{ label: string; value: string }[]>([]);

	const defaultReviewTemplatePath = $derived(
		project.git_host.reviewTemplatePath ?? allAvailableTemplates[0]?.value
	);
	let selectedReviewTemplatePath = $state<string | undefined>(undefined);
	const actualReviewTemplatePath = $derived(
		selectedReviewTemplatePath ?? defaultReviewTemplatePath
	);
</script>

<div class="pr-template__wrap">
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
