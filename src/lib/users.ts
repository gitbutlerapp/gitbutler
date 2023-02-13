import { BaseDirectory, exists, readTextFile, writeTextFile } from '@tauri-apps/api/fs';
import type { User } from '$lib/authentication';
import { writable } from 'svelte/store';
import { dev } from '$app/environment';

const userFile = dev ? 'user.dev.json' : 'user.json';

const isLoggedIn = () => exists(userFile, {
    dir: BaseDirectory.AppLocalData
})

export default async () => {
    const store = writable<User | undefined>(undefined);

    if (await isLoggedIn()) {
        const user = JSON.parse(await readTextFile(userFile, {
            dir: BaseDirectory.AppLocalData
        })) as User;
        store.set(user);
    }

    store.subscribe(async (user) => {
        if (user) {
            await writeTextFile(userFile, JSON.stringify(user), {
                dir: BaseDirectory.AppLocalData
            });
        }
    })

    return store;
}

