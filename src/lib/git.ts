import { Command } from "@tauri-apps/api/shell";

class ExecError extends Error {
    code: number;

    constructor(code: number, message: string) {
        super(message);
        this.code = code;
        this.message = message;
    }
}

export const exec = async (...args: string[]) => {
    const { code, stdout, stderr } = await Command.sidecar(
        "binaries/git",
        args
    ).execute();
    if (code && code !== 0) throw new ExecError(code, stderr);
    return stdout;
};
