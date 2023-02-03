import { database } from "$lib";
import { writable } from "svelte/store";

export type Project = {
    id: string;
    title: string;
    path: string;
};

export const store = async () => {
    const db = await database.json<Project[]>("projects.json");
    const fromDisk = await db.read();
    const store = writable<Project[]>(fromDisk || []);
    store.subscribe(db.write);
    return store;
};
