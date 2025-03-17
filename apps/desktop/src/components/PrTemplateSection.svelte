<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { TemplateService } from '$lib/forge/templateService';
	import { Project } from '$lib/project/project';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';

	interface Props {
		templates: string[];
		onselected: (body: string) => void;
	}

	const { templates, onselected }: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	// TODO: Rename or refactor this service.
	const templateService = getContext(TemplateService);
	const project = getContext(Project);

	// The last template that was used. It is used as default if it is in the
	// list of available commits.
	const lastTemplate = persisted<string | undefined>(undefined, `last-template-${project.id}`);

	async function setTemplate(path: string) {
		lastTemplate.set(path);
		loadAndEmit(path);
	}

	async function loadAndEmit(path: string) {
		if (path) {
			const template = await templateService.getContent(forge.current.name, path);
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
		wide
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
