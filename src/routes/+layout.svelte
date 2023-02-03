<script lang="ts">
    import "../app.postcss";

    import { open } from "@tauri-apps/api/dialog";
    import type { LayoutData } from "./$types";
    import { nanoid } from "nanoid";
    import { path } from "@tauri-apps/api";

    export let data: LayoutData;
    const projects = data.projects;

    const onSelectProjectClick = async () => {
        const selectedPath = await open({
            directory: true,
            recursive: true,
        });
        if (selectedPath === null) return;
        if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
        const projectPath = Array.isArray(selectedPath)
            ? selectedPath[0]
            : selectedPath;

        const projectExists = $projects.some((p) => p.path === projectPath);
        if (projectExists) return;

        const title = await path.basename(projectPath);
        $projects = [
            ...$projects,
            {
                id: nanoid(),
                title,
                path: projectPath,
            },
        ];
    };
</script>

<header data-tauri-drag-region class="h-6">
</header>

<main class="p-2">
    <nav class="flex flex-row m-2">
        <ul class="flex-1 flex flex-row gap-2 overflow-x-scroll">
            {#each $projects as project}
                <li class="border rounded-md bg-blue-100 p-2">
                    <a href="/projects/{project.id}/">{project.title}</a>
                </li>
            {/each}
        </ul>

        <button
            class="rounded-lg bg-green-100 p-1 m-1"
            on:click={onSelectProjectClick}
            type="button"
        >
            new project
        </button>
    </nav>
    <slot />
</main>
