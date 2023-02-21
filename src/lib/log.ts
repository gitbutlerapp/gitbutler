export { attachConsole as setup } from "tauri-plugin-log-api";

import * as log from "tauri-plugin-log-api";

const toString = (value: any) => {
    if (value instanceof Error) {
        return value.message;
    } else if (typeof value === "object") {
        return JSON.stringify(value);
    } else {
        return value.toString();
    }
};

export const debug = (...args: any[]) =>
    log.debug(args.map(toString).join(" "));

export const info = (...args: any[]) => log.info(args.map(toString).join(" "));

export const error = (...args: any[]) =>
    log.error(args.map(toString).join(" "));
