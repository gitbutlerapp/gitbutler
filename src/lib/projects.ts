import { database } from "$lib";
import { writable, type Readable } from "svelte/store";
import { TextDocument } from "./crdt";
import { readFile, readDir, NoSuchFileOrDirectoryError } from "./tauri";
import { EventType, type Event, watch as fsWatch } from "./watch";

export type Project = {
    id: string;
    title: string;
    path: string;
};

export const watch = (
    project: Project
): Readable<Record<string, TextDocument>> => {
    const tree = writable<Record<string, TextDocument>>({});

    // TODO (NB: we can probably use git ls-files)
    const shouldIgnore = (filepath: string) => {
        if (filepath.includes(".git")) return true;
        if (filepath.includes("node_modules")) return true;
        if (filepath.includes("env")) return true;
        if (filepath.includes("__pycache__")) return true;
        return false;
    };

    const upsertDoc = async (filepath: string) => {
        if (shouldIgnore(filepath)) return;

        const content = await readFile(filepath).catch((err) => {
            if (err instanceof NoSuchFileOrDirectoryError) {
                return undefined;
            } else {
                throw err;
            }
        });

        tree.update((tree) => {
            if (content === undefined) {
                delete tree[filepath];
                return tree;
            } else if (filepath in tree) {
                tree[filepath].update(content);
                return tree;
            } else {
                tree[filepath] = TextDocument.new(content);
                return tree;
            }
        });
    };

    readDir(project.path).then((filepaths) => filepaths.forEach(upsertDoc));

    fsWatch(project.path, async (event: Event) => {
        const isFileCreate =
            EventType.isCreate(event.type) && event.type.create.kind === "file";
        const isFileUpdate =
            EventType.isModify(event.type) && event.type.modify.kind === "data";
        const isFileRemove = EventType.isRemove(event.type);

        if (isFileCreate || isFileUpdate) {
            for (const path of event.paths) {
                await upsertDoc(path);
            }
        } else if (isFileRemove) {
            tree.update((tree) => {
                for (const path of event.paths) {
                    delete tree[path];
                }
                return tree;
            });
        }
    });

    return tree;
};

export const store = async () => {
    const db = await database.json<Project[]>("projects.json");
    const fromDisk = await db.read();
    const store = writable<Project[]>(fromDisk || []);
    store.subscribe(db.write);
    return store;
};
