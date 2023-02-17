<script lang="ts">
    import { derived } from "svelte/store";
    import { Login } from "$lib/components";
    import type { PageData } from "./$types";

    export let data: PageData;
    const { project, user, api } = data;

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

<article class="">
    <header>
        <h2>{$project?.title}</h2>
    </header>

    {#if $user}
        <form disabled={$user === undefined}>
            <label for="sync">Sync</label>
            <input
                disabled={$user === undefined}
                type="checkbox"
                checked={$isSyncing}
                on:change={onSyncChange}
            />
        </form>
    {:else}
        <div><Login {user} {api} /> to sync</div>
    {/if}

    <code class="whitespace-pre">
        {JSON.stringify($project, null, 2)}
    </code>
</article>
