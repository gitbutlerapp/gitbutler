<script lang="ts" module>
	export interface HunkContextItem {
		hunk: Hunk;
		beforeLineNumber: number | undefined;
		afterLineNumber: number | undefined;
		section: ContentSection;
	}
</script>

<script lang="ts">
	import { vscodePath } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import type { Hunk } from '$lib/hunks/hunk';
	import type { ContentSection } from '$lib/utils/fileSections';

	interface Props {
		trigger: HTMLElement | undefined;
		filePath: string;
		readonly: boolean;
		projectId: string;
	}

	const { trigger, filePath, readonly, projectId }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const userSettings = inject(SETTINGS);
	const projectService = inject(PROJECTS_SERVICE);

	let contextMenu: ReturnType<typeof ContextMenu> | undefined;

	function getDiscardLineLabel(
		beforeLineNumber: number | undefined,
		afterLineNumber: number | undefined
	) {
		if (beforeLineNumber !== undefined && afterLineNumber !== undefined)
			return `Discard line ${beforeLineNumber} -> ${afterLineNumber}`;
		if (beforeLineNumber !== undefined) return `Discard old line ${beforeLineNumber}`;

		if (afterLineNumber !== undefined) return `Discard new line ${afterLineNumber}`;

		return 'Discard line';
	}

	export function open(e: MouseEvent, item: HunkContextItem) {
		contextMenu?.open(e, item);
	}

	export function close() {
		contextMenu?.close();
	}
</script>

<ContextMenu bind:this={contextMenu} rightClickTrigger={trigger}>
	{#snippet children(item)}
		<ContextMenuSection>
			{#if item.hunk !== undefined && !readonly}
				<ContextMenuItem
					label="Discard change"
					onclick={async () => {
						stackService.legacyUnapplyHunk({ projectId, hunk: item.hunk });
						contextMenu?.close();
					}}
				/>
			{/if}
			{#if item.hunk !== undefined && (item.beforeLineNumber !== undefined || item.afterLineNumber !== undefined) && !readonly}
				<ContextMenuItem
					label={getDiscardLineLabel(item.beforeLineNumber, item.afterLineNumber)}
					onclick={async () => {
						stackService.legacyUnapplyLines({
							projectId,
							hunk: item.hunk,
							linesToUnapply: [{ old: item.beforeLineNumber, new: item.afterLineNumber }]
						});
						contextMenu?.close();
					}}
				/>
			{/if}
			{#if item.beforeLineNumber !== undefined || item.afterLineNumber !== undefined}
				<ContextMenuItem
					label="Open in {$userSettings.defaultCodeEditor.displayName}"
					onclick={async () => {
						const project = await projectService.fetchProject(projectId);
						if (project.path) {
							const path = getEditorUri({
								schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
								path: [vscodePath(project.path), filePath],
								line: item.beforeLineNumber ?? item.afterLineNumber
							});
							openExternalUrl(path);
						}
						contextMenu?.close();
					}}
				/>
			{/if}
		</ContextMenuSection>
	{/snippet}
</ContextMenu>
