import { invoke } from "@tauri-apps/api";

export const readFile = (filePath: string) =>
    invoke<string>("read_file", { filePath });
