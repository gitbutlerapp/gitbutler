import { watchImmediate } from "tauri-plugin-fs-watch-api";

export type EventTypeModify = {
    modify:
    | { kind: "metadata"; mode: "ownership" | "any" }
    | { kind: "data"; mode: "content" };
};

export type EventTypeRemove = {
    remove: {
        kind: "file" | "folder";
    };
};

export type EventTypeCreate = {
    create: {
        kind: "file" | "folder";
    };
};

export type EventType =
    | EventTypeCreate
    | EventTypeRemove
    | EventTypeModify
    | any;

export namespace EventType {
    export const isCreate = (
        eventType: EventType
    ): eventType is EventTypeCreate =>
        (eventType as EventTypeCreate).create !== undefined;

    export const isRemove = (
        eventType: EventType
    ): eventType is EventTypeRemove =>
        (eventType as EventTypeRemove).remove !== undefined;

    export const isModify = (
        eventType: EventType
    ): eventType is EventTypeModify =>
        (eventType as EventTypeModify).modify !== undefined;
}

export type Event = {
    type: EventType;
    paths: string[];
};

export const watch = (
    path: string | string[],
    onEvent: (event: Event) => void
) => watchImmediate(path, { recursive: true }, onEvent);
