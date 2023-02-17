import { invoke } from "@tauri-apps/api";
import { derived, writable } from "svelte/store";
import type { Project as ApiProject } from "$lib/api";

export type Project = {
    id: string;
    title: string;
    path: string;
    api: ApiProject & { sync: boolean };
};

const list = () => invoke<Project[]>("list_projects");

const update = (params: {
    project: {
        id: string;
        title?: string;
        api?: ApiProject & { sync: boolean };
    };
}) => invoke<Project>("update_project", params);

const add = (params: { path: string }) =>
    invoke<Project>("add_project", params);

export default async () => {
    const init = await list();
    const store = writable<Project[]>(init);

    return {
        subscribe: store.subscribe,
        get: (id: string) => {
            const project = derived(store, (store) =>
                store.find((p) => p.id === id)
            );
            return {
                subscribe: project.subscribe,
                update: (params: { title?: string; api?: Project["api"] }) =>
                    update({
                        project: {
                            id,
                            ...params,
                        },
                    }).then((project) => {
                        store.update((projects) =>
                            projects.map((p) =>
                                p.id === project.id ? project : p
                            )
                        );
                        return project;
                    }),
            };
        },
        add: (params: { path: string }) =>
            add(params).then((project) => {
                store.update((projects) => [...projects, project]);
                return project;
            }),
    };
};
