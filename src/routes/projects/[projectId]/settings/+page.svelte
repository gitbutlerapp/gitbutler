<script lang="ts">
    import type { PageData } from "./$types";
    import Authentication from "$lib/authentication";

    const authApi = Authentication();

    export let data: PageData;
    $: project = data.project;
    $: projects = data.projects;
    $: user = data.user;

    function enableSync() {
      const apiProject = authApi.project.create($user.access_token, {name: $project?.title})
        .then((res) => {
            console.log(res);
            if($project) {
              $project.git_url = res.git_url;
              $project.sync = true;
            }
        })
        .catch(() => null);
    }

    function disableSync() {
      if($project) {
        console.log($project);
        $project.sync = false;
      }
    }
</script>

<div class="flex flex-col justify-between p-4">
  <div>
    ID: {$project?.id}
  </div>
  <div>
    Title: {$project?.title}
  </div>
  <div>
    {#if $project?.git_url}
      Syncing to URL: {$project?.git_url}
      <button on:click={disableSync}>Disable Sync</button>
    {:else}
      {#if $user}
        <button on:click={enableSync}>Enable Sync</button>
      {:else}
        Log in to Sync
      {/if}
    {/if}
  </div>
</div>

<slot />