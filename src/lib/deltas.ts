import { invoke } from "@tauri-apps/api";
import { appWindow } from "@tauri-apps/api/window";
import { writable, type Readable } from "svelte/store";

export type OperationDelete = { delete: [number, number] };
export type OperationInsert = { insert: [number, string] };

export type Operation = OperationDelete | OperationInsert;

export namespace Operation {
    export const isDelete = (
        operation: Operation
    ): operation is OperationDelete => "delete" in operation;

    export const isInsert = (
        operation: Operation
    ): operation is OperationInsert => "insert" in operation;
}

export type Delta = { timestampMs: number; operations: Operation[] };

type DeltasEvent = {
    deltas: Delta[];
    filePath: string;
};

const list = (params: { projectId: string; sessionId: string }) =>
    invoke<Record<string, Delta[]>>("list_deltas", params);

export default async (params: { projectId: string, sessionId: string }) => {
    const init = await list(params);

    const store = writable<Record<string, Delta[]>>(init);
    const eventName = `project://${params.projectId}/sessions/${params.sessionId}/deltas`;
    await appWindow.listen<DeltasEvent>(eventName, (event) => {
        store.update((deltas) => ({
            ...deltas,
            [event.payload.filePath]: event.payload.deltas,
        }));
    });

    return store as Readable<Record<string, Delta[]>>;
};
