import { invoke } from "@tauri-apps/api";
import { appWindow } from "@tauri-apps/api/window";
import { writable, type Subscriber } from "svelte/store";

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

const list = (params: { projectId: string }) =>
    invoke<Record<string, Delta[]>>("list_deltas", params);

export default async (params: { projectId: string }) => {
    const files = await invoke<string[]>("list_project_files", params);
    const contents = await Promise.all(
        files.map((filePath) =>
            invoke<string>("read_project_file", { ...params, filePath })
        )
    );

    // this is a temporary workaround to get the initial state of the document
    // TODO: remove this once sessions api is implemented
    const tmpState: Record<string, Delta[]> = Object.fromEntries(
        files.map((filePath, index) => [
            filePath,
            [
                {
                    timestampMs: 0,
                    operations: [{ insert: [0, contents[index]] } as OperationInsert],
                },
            ],
        ])
    );

    const init = await list(params);

    const tmpInit = Object.fromEntries(
        Object.entries(tmpState).map(([filePath, deltas]) => [
            filePath,
            [...deltas, ...(filePath in init ? init[filePath] : [])],
        ])
    );

    const store = writable<Record<string, Delta[]>>(tmpInit);
    const eventName = `deltas://${params.projectId}`;
    const unlisten = await appWindow.listen<DeltasEvent>(eventName, (event) => {
        store.update((deltas) => ({
            ...deltas,
            [event.payload.filePath]: [
                ...(event.payload.filePath in tmpState
                    ? tmpState[event.payload.filePath]
                    : []),
                ...event.payload.deltas,
            ],
        }));
    });
    return {
        subscribe: (
            run: Subscriber<Record<string, Delta[]>>,
            invalidate?: (value?: Record<string, Delta[]>) => void
        ) =>
            store.subscribe(run, (value) => {
                if (invalidate) invalidate(value);
                // unlisten();
            }),
    };
};
