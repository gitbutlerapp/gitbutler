<script lang="ts">
    import { derived } from "svelte/store";
    import { Login } from "$lib/components";
    import type { PageData } from "./$types";
    import hi from "date-fns/locale/hi";

    export let data: PageData;
    const { project, user, api } = data;

    function repo_id(url: string) {
        const hurl = new URL(url);
        const path = hurl.pathname.split("/");
        return path[path.length - 1];
    }

    function hostname(url: string) {
        const hurl = new URL(url);
        return hurl.hostname;
    }

    const isSyncing = derived(project, (project) => project?.api?.sync);

    const onSyncChange = async (event: Event) => {
        if ($project === undefined) return;
        if ($user === undefined) return;

        const target = event.target as HTMLInputElement;
        const sync = target.checked;

        if (!$project.api) {
            const apiProject = await api.projects.create($user.access_token, {
                name: $project.title,
                uid: $project.id,
            });
            await project.update({ api: { ...apiProject, sync } });
        } else {
            await project.update({ api: { ...$project.api, sync } });
        }
    };
</script>

<div class="p-4 mx-auto h-full overflow-auto">
    <div class="max-w-2xl mx-auto p-4">
        <div class="flex flex-col text-zinc-100 space-y-6">
            <div class="space-y-0">
                <div class="text-lg font-medium">Project Settings</div>
                <div class="text-zinc-400">
                    Manage your project settings for <strong
                        >{$project?.title}</strong
                    >
                </div>
            </div>
            <hr class="border-zinc-600" />
            {#if $user}
                <div class="space-y-2">
                    <div class="ml-1">GitButler Cloud</div>
                    <div
                        class="flex flex-row justify-between border border-zinc-600 rounded-lg p-2 items-center"
                    >
                        <div class="flex flex-row space-x-3">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                fill="none"
                                viewBox="0 0 24 24"
                                stroke-width="1.5"
                                stroke="white"
                                class="w-6 h-6"
                            >
                                <path
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    d="M12 16.5V9.75m0 0l3 3m-3-3l-3 3M6.75 19.5a4.5 4.5 0 01-1.41-8.775 5.25 5.25 0 0110.233-2.33 3 3 0 013.758 3.848A3.752 3.752 0 0118 19.5H6.75z"
                                />
                            </svg>
                            <div class="flex flex-row">
                                {#if $project?.api?.git_url}
                                    <div class="flex flex-col">
                                        <div class="text-zinc-300">
                                            Git Host
                                        </div>
                                        <div
                                            class="text-zinc-400 font-mono"
                                        >
                                            {hostname($project?.api?.git_url)}
                                        </div>
                                        <div class="text-zinc-300 mt-3">
                                            Repository ID
                                        </div>
                                        <div
                                            class="text-zinc-400 font-mono"
                                        >
                                            {repo_id($project?.api?.git_url)}
                                        </div>
                                    </div>
                                {/if}
                                <div>
                                    <form disabled={$user === undefined}>
                                        <input
                                            class="mr-1"
                                            disabled={$user === undefined}
                                            type="checkbox"
                                            checked={$isSyncing}
                                            on:change={onSyncChange}
                                        />
                                        <label for="sync"
                                            >Send Data to Server</label
                                        >
                                    </form>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            {:else}
                <div class="space-y-2">
                    <div class="flex flex-row space-x-2 items-end">
                        <div class="">GitButler Cloud</div>
                        <div class="text-zinc-400">
                            backup your work and access advanced features
                        </div>
                    </div>
                    <div class="flex flex-row items-center space-x-2">
                        <Login {user} {api} />
                    </div>
                </div>
            {/if}
            <div class="space-y-2">
                <div class="ml-1">Path</div>
                <div class="text-zinc-400 font-mono">
                    {$project?.path}
                </div>
            </div>
            <div class="space-y-2">
                <div class="ml-1">Project Name</div>
                <!-- text box -->
                <input
                    type="text"
                    class="p-2 text-zinc-300 bg-black border border-zinc-600 rounded-lg w-full"
                    value={$project?.title}
                />
            </div>
            <div class="space-y-2">
                <div class="ml-1">Project Description</div>
                <!-- text box -->
                <textarea
                    rows="3"
                    class="p-2 text-zinc-300 bg-black border border-zinc-600 rounded-lg w-full"
                    value={$project?.api?.description}
                />
            </div>
        </div>
    </div>
</div>
