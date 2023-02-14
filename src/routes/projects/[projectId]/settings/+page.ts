import { derived } from "svelte/store";
import type { PageLoad } from "./$types";
import { building } from "$app/environment";

export const prerender = false;

export const load: PageLoad = async ({ parent, params }) => {
  const { projects } = await parent();
  const user = building ? writable<undefined>(undefined) : await (await import("$lib/users")).default();
  return {
    project: derived(projects, (projects) =>
      projects.find((project) => project.id === params.projectId)
    ),
    projects: projects,
    user: user
  };
};
