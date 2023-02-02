import {
    exists,
    readTextFile,
    BaseDirectory,
    writeTextFile,
    createDir,
} from "@tauri-apps/api/fs";
import { join } from "@tauri-apps/api/path";

const options = {
    dir: BaseDirectory.AppLocalData,
};

export const json = async <T extends any>(filename: string) => {
    await createDir("databases", { ...options, recursive: true });
    const path = await join("databases", filename);
    return {
        read: async (): Promise<T | undefined> => {
            const isExists = await exists(path, {
                dir: BaseDirectory.AppLocalData,
            });
            if (!isExists) {
                return undefined;
            } else {
                const contents = await readTextFile(path, options);
                return JSON.parse(contents) as T;
            }
        },

        write: async (value: T): Promise<void> => {
            await writeTextFile(path, JSON.stringify(value), options);
        },
    };
};
