<script lang="ts">
	import { goto } from "$app/navigation";
	import { BACKEND } from "$lib/backend";
	import { getEditorUri, URL_SERVICE } from "$lib/backend/url";
	import { FILE_SERVICE } from "$lib/files/fileService";
	import { vscodePath } from "$lib/project/project";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { historyPath } from "$lib/routes/routes.svelte";
	import { useSettingsModal } from "$lib/settings/settingsModal.svelte";
	import { SHORTCUT_SERVICE } from "$lib/shortcuts/shortcutService";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { mergeUnlisten } from "@gitbutler/ui/utils/mergeUnlisten";

	const { projectId }: { projectId: string } = $props();

	const backend = inject(BACKEND);
	const projectsService = inject(PROJECTS_SERVICE);
	const urlService = inject(URL_SERVICE);
	const { openProjectSettings } = useSettingsModal();

	const uiState = inject(UI_STATE);
	const shortcutService = inject(SHORTCUT_SERVICE);
	const fileService = inject(FILE_SERVICE);

	$effect(() =>
		mergeUnlisten(
			shortcutService.on("project-settings", () => {
				openProjectSettings(projectId);
			}),
			shortcutService.on("history", () => {
				goto(historyPath(projectId));
			}),
			shortcutService.on("open-in-vscode", async () => {
				const project = await projectsService.fetchProject(projectId);
				if (!project) {
					throw new Error(`Project not found: ${projectId}`);
				}
				urlService.openExternalUrl(
					getEditorUri({
						schemeId: uiState.global.defaultCodeEditor.current.schemeIdentifer,
						path: [vscodePath(project.path)],
						searchParams: { windowId: "_blank" },
					}),
				);
			}),
			shortcutService.on("show-in-finder", async () => {
				const project = await projectsService.fetchProject(projectId);
				if (!project) {
					throw new Error(`Project not found: ${projectId}`);
				}
				// Show the project directory in the default file manager (cross-platform)
				await fileService.showFileInFolder(project.path);
			}),
			shortcutService.on("open-in-terminal", async () => {
				// TODO: once projectId is a project handle, it can be sent to the
				// backend directly and we don't need to fetch.
				const project = await projectsService.fetchProject(projectId);
				if (!project) {
					throw new Error(`Project not found: ${projectId}`);
				}
				await backend.invoke("open_in_terminal", {
					terminalId: uiState.global.defaultTerminal.current.identifier,
					path: project.path,
				});
			}),
		),
	);
</script>
