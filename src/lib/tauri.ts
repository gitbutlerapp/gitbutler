import { invoke } from "@tauri-apps/api";

export class NoSuchFileOrDirectoryError extends Error {
    constructor(message: string) {
        super(message);
    }
}

export const readFile = (filePath: string) =>
    invoke<string>("read_file", { filePath }).catch((err) => {
        if (err.message === "No such file or directory (os error 2)") {
            throw new NoSuchFileOrDirectoryError(err.message);
        } else {
            throw err;
        }
    });
