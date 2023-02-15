import type { User } from "$lib/authentication";
import { writable } from "svelte/store";
import { invoke } from "@tauri-apps/api";

const get = () => invoke<User | undefined>("get_user");

const set = (params: { user: User }) => invoke<void>("set_user", params);

const del = () => invoke<void>("delete_user");

export default async () => {
    const store = writable<User | undefined>(undefined);

    const init = await get();
    store.set(init);
    return {
        subscribe: store.subscribe,
        set: async (user: User) => {
            await set({ user });
            store.set(user);
        },
        delete: async () => {
            await del();
            store.set(undefined);
        },
    };
};
