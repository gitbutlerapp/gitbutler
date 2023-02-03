import { invoke } from "@tauri-apps/api";
import { log } from "$lib";

export class NoSuchFileOrDirectoryError extends Error {
    constructor(message: string) {
        super(message);
    }
}

export const readFile = async (filePath: string) => {
    log.info("readFile", { path: filePath });
    return invoke<string>("read_file", { filePath }).catch((err) => {
        if (err.message === "No such file or directory (os error 2)") {
            throw new NoSuchFileOrDirectoryError(err.message);
        } else {
            throw err;
        }
    });
};

export const readDir = async (path: string) => {
    log.info("readDir", { path });
    return invoke<string[]>("read_dir", { path }).catch((err) => {
        if (err.message === "No such file or directory (os error 2)") {
            throw new NoSuchFileOrDirectoryError(err.message);
        } else {
            throw err;
        }
    });
};
