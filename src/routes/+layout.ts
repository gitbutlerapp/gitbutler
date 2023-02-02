import type { LayoutLoad } from "./$types";
import { projects } from "$lib";

export const ssr = false;
export const prerender = true;
export const csr = true;

export const load: LayoutLoad = () => ({
    projects: projects.store(),
});
