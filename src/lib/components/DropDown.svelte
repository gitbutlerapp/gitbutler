<script lang="ts">
  import { onMount } from "svelte";
  import { scale } from "svelte/transition";
  import type { Project } from "$lib/projects";
  import { page } from "$app/stores";
  import { derived } from "svelte/store";

  export let projects: Project[];

  $: project = projects.find(
    (project: Project) => project.id === $page.params.id
  );

  let show = false;
  let menu: HTMLDivElement | null = null;

  onMount(() => {
    const handleOutsideClick = (event: { target: any }) => {
      if (show && !menu?.contains(event.target)) {
        show = false;
      }
    };

    const handleEscape = (event: { key: string }) => {
      if (show && event.key === "Escape") {
        show = false;
      }
    };

    document.addEventListener("click", handleOutsideClick, false);
    document.addEventListener("keyup", handleEscape, false);

    console.log($page.params);
    return () => {
      document.removeEventListener("click", handleOutsideClick, false);
      document.removeEventListener("keyup", handleEscape, false);
    };
  });
</script>

<div
  class="relative w-full"
  bind:this={menu}
>
  <div class="flex justify-center items-start">
    <button
      on:click={() => (show = !show)}
      class="menu focus:outline-none focus:shadow-solid cursor-default flex-grow"
    >
      <div class="text-left px-4">
        {#if project}
          {project?.title}
        {:else}
          Repositories
        {/if}
      </div>
    </button>

    {#if show}
      <div
        in:scale={{ duration: 100, start: 0.95 }}
        out:scale={{ duration: 75, start: 0.95 }}
        class="absolute origin-top w-[90%] py-2 mt-7
         bg-zinc-700
         border border-zinc-600
            rounded"
      >
        {#each projects as project}
          <a
            class="block cursor-default px-4 py-2 hover:bg-zinc-900"
            on:click={() => show = false}
            href="/projects/{project.id}/">{project.title}</a
          >
        {/each}
      </div>
    {/if}
  </div>
</div>
