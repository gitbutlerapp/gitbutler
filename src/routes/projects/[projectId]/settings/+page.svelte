<script lang="ts">
    import type { PageData } from "./$types";
    import type { Project } from "$lib/projects";
    import Authentication from "$lib/authentication";
    import { BaseDirectory, writeTextFile } from '@tauri-apps/api/fs';
    import Projects from '$lib/projects';

    const authApi = Authentication();
    const projectsFile = 'projects.json';

    export let data: PageData;
    $: project = data.project;
    $: projects = data.projects;
    $: user = data.user;

    Projects().then((projectStore) => {
      projectStore.subscribe((newProjects) => {
        // write to the disk
        console.log("Projects Store Updated");
        console.log(newProjects);
      })
    });

    function updateProject(project: Project) {
        console.log("Update Project", project);
        const items = $projects;
        const newProjects = items.map(item => item.id === project.id ? project : item);

        console.log(newProjects);
        // omg

        writeTextFile(projectsFile, JSON.stringify(newProjects), {
            dir: BaseDirectory.AppLocalData
        });
        $projects = newProjects;
    };

    function enableSync() {
      const apiProject = authApi.project.create($user.access_token, {name: $project?.title})
        .then((res) => {
            console.log("Git response");
            console.log(res);
            if($project) {
              console.log("Updating Project");
              $project.git_url = res.git_url;
              console.log("Set git url");
              $project.sync = true;
              console.log("Set sync");
              console.log($project);
              updateProject($project);
            }
        })
        .catch((e) => console.log("error", e));
    }

    function disableSync() {
      if($project) {
        console.log($project);
        $project.git_url = null;
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