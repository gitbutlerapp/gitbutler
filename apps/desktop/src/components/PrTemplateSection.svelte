<script lang="ts">
	import Select from '$components/Select.svelte';
	import SelectItem from '$components/SelectItem.svelte';
	import { getForge } from '$lib/forge/interface/forge';
	import { TemplateService } from '$lib/pr/templateService';
	import { Project } from '$lib/project/project';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';

	interface Props {
		templates: string[];
		onselected: (body: string) => void;
	}

	const { templates, onselected }: Props = $props();

	const forge = getForge();
	// TODO: Rename or refactor this service.
	const templateService = getContext(TemplateService);
	const project = getContext(Project);

	// The last template that was used. It is used as default if it is in the
	// list of available commits.
	const lastTemplate = persisted<string | undefined>(undefined, `last-template-${project.id}`);

	async function setTemplate(path: string) {
		if ($forge) {
			lastTemplate.set(path);
			loadAndEmit(path);
		}
	}

	async function loadAndEmit(path: string) {
		if (path && $forge) {
			const template = await templateService.getContent($forge.name, path);
			if (template) {
				onselected(template);
			}
		}
	}

	$effect(() => {
		if (templates) {
			if ($lastTemplate && templates.includes($lastTemplate)) {
				loadAndEmit($lastTemplate);
			} else if (templates.length === 1) {
				const path = templates.at(0);
				if (path) {
					loadAndEmit(path);
					lastTemplate.set(path);
				}
			}
		}
	});
</script>

<div class="pr-template__wrap">
	<Select
		value={$lastTemplate}
		options={templates.map((value) => ({ label: value, value }))}
		placeholder={templates.length > 0 ? 'Choose template' : 'No PR templates found ¯_(ツ)_/¯'}
		flex="1"
		searchable
		disabled={templates.length === 0}
		onselect={setTemplate}
	>
		{#snippet itemSnippet({ item, highlighted })}
			<SelectItem selected={item.value === $lastTemplate} {highlighted}>
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
