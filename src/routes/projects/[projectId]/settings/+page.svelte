<script lang="ts">
    import type { PageData } from "./$types";
    import type { Project } from "$lib/projects";
    import Authentication from "$lib/authentication";
    import { BaseDirectory, writeTextFile, removeFile, exists } from '@tauri-apps/api/fs';
  	import { beforeUpdate, afterUpdate } from 'svelte';
    import { invoke } from "@tauri-apps/api";

    const authApi = Authentication();

    export let data: PageData;
    $: project = data.project;
    $: user = data.user;

    let projectPath = "project-" + $project?.id.toString() + ".json"
    $: pathExists = false;

    afterUpdate(() => {
      projectPath = "project-" + $project?.id.toString() + ".json"
      checkExists();

    })

    function checkExists() {
      exists(projectPath, {
          dir: BaseDirectory.AppLocalData
      }).then((res) => {
          pathExists = res;
      }).catch((e) => {
          console.log("Error checking for file");
          console.log(e);
      });
    }

    function updateProject(project: Project, url) {
        writeTextFile(projectPath, JSON.stringify({url: url}), {
            dir: BaseDirectory.AppLocalData
        }).then(() => {
            console.log("Wrote URL");
            projectPath = "project-" + $project?.id.toString() + ".json"
            invoke("add_git_url", { url: url, path: $project?.path }).then((res) => {
              console.log("Added git url");
              console.log(res);
            }).catch((e) => {
              console.log("Error adding git url");
              console.log(e);
            })
            checkExists();
        }).catch((e) => {
            console.log("Error writing URL");
            console.log(e);
        });
    };

    function enableSync() {
      const apiProject = authApi.project.create($user.access_token, {name: $project?.title, uid: $project?.id})
        .then((res) => {
            console.log("Git response");
            console.log(res);
            if($project) {
              console.log("Updating Project");
              console.log($project);
              updateProject($project, res.git_url);
            }
        })
        .catch((e) => console.log("error", e));
    }

    function disableSync() {
      if($project) {
        console.log($project);
        removeFile(projectPath, {
            dir: BaseDirectory.AppLocalData
        }).then(() => {
            console.log("Removed URL");
            invoke("remove_git_url", { path: $project?.path }).then((res) => {
              console.log("Removed git url");
              console.log(res);
            }).catch((e) => {
              console.log("Error removing git url");
              console.log(e);
            })
            checkExists();
        }).catch((e) => {
            console.log("Error removing URL");
            console.log(e);
        });
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
    Exists: {pathExists}<br/>
    Path: {projectPath}<br/>
    {#if pathExists}
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