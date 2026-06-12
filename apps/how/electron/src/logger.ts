import fs from "node:fs/promises";
import path from "node:path";
import { inspect } from "node:util";

export type Logger = {
	info(message: string, data?: unknown): void;
	error(message: string, error?: unknown, data?: unknown): void;
};

function serialize(value: unknown): string {
	if (value === undefined) return "";
	if (value instanceof Error) return `${value.stack ?? value.message}`;
	if (typeof value === "string") return value;
	return inspect(value, { depth: 8, colors: false, breakLength: 140 });
}

export function createLogger(logPath: string): Logger {
	async function append(level: "info" | "error", message: string, data?: unknown): Promise<void> {
		const line = [
			new Date().toISOString(),
			level.toUpperCase(),
			message,
			serialize(data),
		]
			.filter((part) => part.length > 0)
			.join(" ");

		if (level === "error") console.error(line);
		else console.warn(line);

		await fs.mkdir(path.dirname(logPath), { recursive: true });
		await fs.appendFile(logPath, `${line}\n`);
	}

	return {
		info(message, data) {
			void append("info", message, data);
		},
		error(message, error, data) {
			void append("error", message, { error: serialize(error), data });
		},
	};
}
